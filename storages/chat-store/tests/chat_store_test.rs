//! Integration tests: real SqliteStore (in-memory), real writer task, events
//! fed through the public handler exactly as the client would.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use std::sync::Arc;
use std::time::Duration;

use buffa::MessageField;
use chrono::{Datelike, TimeZone, Utc};
use diesel::RunQueryDsl;
use wacore::proto_helpers::MessageBuilderExt;
use wacore::types::events::{
    BatchOrigin, Event, InboundMessage, LazyHistorySync, MessageBatch, Receipt, ServerAck,
};
use wacore::types::message::{MessageInfo, MessageSource};
use wacore::types::presence::ReceiptType;
use wacore_binary::Jid;
use waproto::whatsapp as wa;
use whatsapp_rust_chat_store::{ChatStore, MessageKind, MessageStatus, StoreChange};
use whatsapp_rust_sqlite_storage::SqliteStore;

const PEER: &str = "559900000001@s.whatsapp.net";
const GROUP: &str = "120363000000000001@g.us";

async fn test_store() -> (SqliteStore, Arc<ChatStore>) {
    use portable_atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let db_name = format!(
        "file:memdb_chat_store_{}_{}?mode=memory&cache=shared",
        std::process::id(),
        id
    );
    let store = SqliteStore::new(&db_name).await.expect("create store");
    let chat_store = ChatStore::new(&store).await.expect("create chat store");
    (store, chat_store)
}

fn jid(s: &str) -> Jid {
    s.parse().expect("valid test JID")
}

fn incoming_info(chat: &str, sender: &str, id: &str, ts_secs: i64) -> MessageInfo {
    MessageInfo {
        source: MessageSource {
            chat: jid(chat),
            sender: jid(sender),
            is_from_me: false,
            is_group: chat.ends_with("@g.us"),
            ..Default::default()
        },
        id: id.to_string(),
        timestamp: Utc.timestamp_opt(ts_secs, 0).unwrap(),
        ..Default::default()
    }
}

fn message_event(msg: wa::Message, info: MessageInfo) -> Event {
    Event::Messages(
        MessageBatch::builder()
            .messages(Arc::from([InboundMessage::builder()
                .message(Arc::new(msg))
                .info(Arc::new(info))
                .build()]))
            .origin(BatchOrigin::Live)
            .build(),
    )
}

async fn feed(chat_store: &ChatStore, events: impl IntoIterator<Item = Event>) {
    let handler = chat_store.handler();
    for event in events {
        handler.handle_event(Arc::new(event));
    }
    chat_store.flush().await.expect("flush");
}

fn history_sync_event(history: wa::HistorySync) -> Event {
    use buffa::Message as _;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    let raw = history.encode_to_vec();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw).unwrap();
    Event::HistorySync(Box::new(LazyHistorySync::new(
        enc.finish().unwrap().into(),
        raw.len(),
        wa::history_sync::HistorySyncType::RECENT as i32,
        None,
        None,
    )))
}

#[tokio::test]
async fn live_text_message_materializes_chat_and_message() {
    let (_store, chat_store) = test_store().await;

    let mut info = incoming_info(PEER, PEER, "MSG-1", 1_700_000_000);
    info.push_name = "Alice Example".into();
    feed(&chat_store, [message_event(wa::Message::text("olá"), info)]).await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("olá"));
    assert_eq!(chats[0].unread_count, 1);

    let messages = chat_store.messages(&jid(PEER), None, 10).await.unwrap();
    assert_eq!(messages.len(), 1);
    let msg = &messages[0];
    assert_eq!(msg.id, "MSG-1");
    assert_eq!(msg.kind, MessageKind::Text);
    assert_eq!(msg.text.as_deref(), Some("olá"));
    assert!(!msg.from_me);
    // The stored proto round-trips.
    let proto = msg.message.as_ref().expect("decoded proto");
    assert_eq!(proto.conversation.as_deref(), Some("olá"));

    // Live push name landed in contacts.
    let contact = chat_store.contact(&jid(PEER)).await.unwrap().unwrap();
    assert_eq!(contact.push_name.as_deref(), Some("Alice Example"));
    assert_eq!(contact.display_name(), Some("Alice Example"));
}

#[tokio::test]
async fn business_verified_name_is_learned_from_live_messages() {
    let (_store, chat_store) = test_store().await;

    let mut info = incoming_info(PEER, PEER, "MSG-BIZ-1", 1_700_000_000);
    info.verified_name = Some(Box::new(wacore::stanza::business::VerifiedName {
        name: Some("Fictitious Biz Ltd".into()),
        serial: Some("12345".into()),
        issuer: Some("smb:wa".into()),
        certificate: None,
    }));
    feed(
        &chat_store,
        [message_event(wa::Message::text("promo"), info)],
    )
    .await;

    let contact = chat_store.contact(&jid(PEER)).await.unwrap().unwrap();
    assert_eq!(contact.business_name.as_deref(), Some("Fictitious Biz Ltd"));
    // No address-book or push name: the verified name is the display name.
    assert_eq!(contact.display_name(), Some("Fictitious Biz Ltd"));
}

#[tokio::test]
async fn later_message_without_verified_name_keeps_business_name() {
    let (_store, chat_store) = test_store().await;

    let mut info = incoming_info(PEER, PEER, "MSG-BIZ-2", 1_700_000_000);
    info.verified_name = Some(Box::new(wacore::stanza::business::VerifiedName {
        name: Some("Fictitious Biz Ltd".into()),
        serial: None,
        issuer: None,
        certificate: None,
    }));
    let plain = incoming_info(PEER, PEER, "MSG-BIZ-3", 1_700_000_100);
    feed(
        &chat_store,
        [
            message_event(wa::Message::text("promo"), info),
            message_event(wa::Message::text("follow-up"), plain),
        ],
    )
    .await;

    let contact = chat_store.contact(&jid(PEER)).await.unwrap().unwrap();
    assert_eq!(contact.business_name.as_deref(), Some("Fictitious Biz Ltd"));
}

#[tokio::test]
async fn nameless_verified_cert_creates_no_contact_row() {
    let (_store, chat_store) = test_store().await;

    let mut info = incoming_info(PEER, PEER, "MSG-BIZ-4", 1_700_000_000);
    info.verified_name = Some(Box::new(wacore::stanza::business::VerifiedName {
        name: None,
        serial: None,
        issuer: None,
        certificate: Some(vec![0xff, 0x13]),
    }));
    feed(&chat_store, [message_event(wa::Message::text("hi"), info)]).await;

    assert!(chat_store.contact(&jid(PEER)).await.unwrap().is_none());
}

#[tokio::test]
async fn outgoing_status_advances_monotonically() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let local_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();

    chat_store
        .record_outgoing(&chat, "OUT-1", &wa::Message::text("oi"), local_timestamp)
        .unwrap();
    chat_store.flush().await.unwrap();
    let msg = chat_store.message(&chat, "OUT-1").await.unwrap().unwrap();
    assert!(msg.from_me);
    assert_eq!(msg.status, MessageStatus::Pending);

    // Server ack lifts to ServerAck.
    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-1".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .build(),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "OUT-1").await.unwrap().unwrap();
    assert_eq!(msg.status, MessageStatus::ServerAck);
    assert_eq!(msg.timestamp, local_timestamp);

    // Read receipt from the peer.
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: chat.clone(),
                    sender: chat.clone(),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-1".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_200, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "OUT-1").await.unwrap().unwrap();
    assert_eq!(msg.status, MessageStatus::Read);

    // A late Delivered must NOT downgrade Read.
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: chat.clone(),
                    sender: chat.clone(),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-1".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                .r#type(ReceiptType::Delivered)
                .offline(false)
                .build(),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "OUT-1").await.unwrap().unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

#[tokio::test]
async fn server_ack_reconciles_outgoing_timestamp_and_thread_order() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let server_timestamp = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let reply_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
    let local_timestamp = Utc.timestamp_opt(1_700_000_200, 0).unwrap();

    chat_store
        .record_outgoing(
            &chat,
            "OUT-CLOCK",
            &wa::Message::text("question"),
            local_timestamp,
        )
        .unwrap();
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("reply"),
            incoming_info(PEER, PEER, "REPLY-CLOCK", reply_timestamp.timestamp()),
        )],
    )
    .await;

    let before = chat_store.messages(&chat, None, 10).await.unwrap();
    assert_eq!(before[0].id, "OUT-CLOCK");
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_at, Some(local_timestamp));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("question"));

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-CLOCK".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;

    let after = chat_store.messages(&chat, None, 10).await.unwrap();
    assert_eq!(
        after
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["REPLY-CLOCK", "OUT-CLOCK"]
    );
    assert_eq!(after[1].timestamp, server_timestamp);
    assert_eq!(after[1].status, MessageStatus::ServerAck);

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_at, Some(reply_timestamp));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("reply"));
}

#[tokio::test]
async fn server_ack_can_move_outgoing_message_to_thread_head() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let local_timestamp = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let reply_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
    let server_timestamp = Utc.timestamp_opt(1_700_000_200, 0).unwrap();

    chat_store
        .record_outgoing(
            &chat,
            "OUT-CLOCK-FORWARD",
            &wa::Message::text("question"),
            local_timestamp,
        )
        .unwrap();
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("reply"),
            incoming_info(
                PEER,
                PEER,
                "REPLY-CLOCK-FORWARD",
                reply_timestamp.timestamp(),
            ),
        )],
    )
    .await;

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-CLOCK-FORWARD".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;

    let messages = chat_store.messages(&chat, None, 10).await.unwrap();
    assert_eq!(messages[0].id, "OUT-CLOCK-FORWARD");
    assert_eq!(messages[0].timestamp, server_timestamp);
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_at, Some(server_timestamp));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("question"));
}

#[tokio::test]
async fn server_ack_reconciles_timestamp_after_receipt_advanced_status() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let server_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();

    chat_store
        .record_outgoing(
            &chat,
            "OUT-RECEIPT-FIRST",
            &wa::Message::text("hello"),
            Utc.timestamp_opt(1_700_000_200, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [
            Event::Receipt(
                Receipt::builder()
                    .source(MessageSource {
                        chat: chat.clone(),
                        sender: chat.clone(),
                        ..Default::default()
                    })
                    .message_ids(vec!["OUT-RECEIPT-FIRST".to_string()])
                    .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                    .r#type(ReceiptType::Delivered)
                    .offline(false)
                    .build(),
            ),
            Event::ServerAck(
                ServerAck::builder()
                    .id("OUT-RECEIPT-FIRST".to_string())
                    .class("message".to_string())
                    .from(chat.clone())
                    .timestamp(server_timestamp)
                    .build(),
            ),
        ],
    )
    .await;

    let msg = chat_store
        .message(&chat, "OUT-RECEIPT-FIRST")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Delivered);
    assert_eq!(msg.timestamp, server_timestamp);
}

#[tokio::test]
async fn server_nack_does_not_reconcile_timestamp() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let local_timestamp = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

    chat_store
        .record_outgoing(
            &chat,
            "OUT-NACK-CLOCK",
            &wa::Message::text("hello"),
            local_timestamp,
        )
        .unwrap();
    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-NACK-CLOCK".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .timestamp(Utc.timestamp_opt(1_700_000_500, 0).unwrap())
                .error("479".to_string())
                .build(),
        )],
    )
    .await;

    let msg = chat_store
        .message(&chat, "OUT-NACK-CLOCK")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Error);
    assert_eq!(msg.timestamp, local_timestamp);
}

#[tokio::test]
async fn server_ack_disambiguates_reused_message_id_by_chat() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);
    let group = jid(GROUP);
    let peer_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
    let group_timestamp = Utc.timestamp_opt(1_700_000_200, 0).unwrap();
    let server_timestamp = Utc.timestamp_opt(1_700_000_300, 0).unwrap();
    let shared_id = "OUT-SHARED-ID";

    chat_store
        .record_outgoing(&peer, shared_id, &wa::Message::text("peer"), peer_timestamp)
        .unwrap();
    chat_store
        .record_outgoing(
            &group,
            shared_id,
            &wa::Message::text("group"),
            group_timestamp,
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    // Without a usable chat identity, a duplicate id is ambiguous and must
    // update neither row.
    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id(shared_id.to_string())
                .class("message".to_string())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;
    for (chat, timestamp) in [(&peer, peer_timestamp), (&group, group_timestamp)] {
        let msg = chat_store.message(chat, shared_id).await.unwrap().unwrap();
        assert_eq!(msg.status, MessageStatus::Pending);
        assert_eq!(msg.timestamp, timestamp);
    }

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id(shared_id.to_string())
                .class("message".to_string())
                .from(group.clone())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;

    let peer_msg = chat_store.message(&peer, shared_id).await.unwrap().unwrap();
    assert_eq!(peer_msg.status, MessageStatus::Pending);
    assert_eq!(peer_msg.timestamp, peer_timestamp);
    let group_msg = chat_store
        .message(&group, shared_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group_msg.status, MessageStatus::ServerAck);
    assert_eq!(group_msg.timestamp, server_timestamp);
}

#[tokio::test]
async fn server_ack_refreshes_preview_behind_retained_activity_timestamp() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let server_timestamp = Utc.timestamp_opt(1_700_000_050, 0).unwrap();
    let survivor_timestamp = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
    let local_timestamp = Utc.timestamp_opt(1_700_000_200, 0).unwrap();
    let deleted_timestamp = Utc.timestamp_opt(1_700_000_300, 0).unwrap();

    chat_store
        .record_outgoing(
            &chat,
            "OUT-RETAINED-HEAD",
            &wa::Message::text("question"),
            local_timestamp,
        )
        .unwrap();
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("survivor"),
                incoming_info(PEER, PEER, "MSG-SURVIVOR", survivor_timestamp.timestamp()),
            ),
            message_event(
                wa::Message::text("delete me"),
                incoming_info(
                    PEER,
                    PEER,
                    "MSG-DELETED-HEAD",
                    deleted_timestamp.timestamp(),
                ),
            ),
        ],
    )
    .await;
    feed(
        &chat_store,
        [Event::DeleteMessageForMeUpdate(
            wacore::types::events::DeleteMessageForMeUpdate::builder()
                .chat_jid(chat.clone())
                .message_id("MSG-DELETED-HEAD".to_string())
                .from_me(false)
                .timestamp(Utc.timestamp_opt(1_700_000_400, 0).unwrap())
                .action(Box::new(
                    wa::sync_action_value::DeleteMessageForMeAction::default(),
                ))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_at, Some(deleted_timestamp));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("question"));

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-RETAINED-HEAD".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_at, Some(deleted_timestamp));
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("survivor"));
}

#[tokio::test]
async fn edit_updates_and_revoke_tombstones() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("typo"),
            incoming_info(PEER, PEER, "MSG-E", 1_700_000_000),
        )],
    )
    .await;

    // Edit arrives as protocolMessage MESSAGE_EDIT targeting the original id.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-E".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("fixed"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-E2", 1_700_000_050),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "MSG-E").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("fixed"));
    assert!(msg.edited_at.is_some());
    assert!(!msg.revoked);
    // The edit protocol message itself must not create a bubble row.
    assert!(chat_store.message(&chat, "MSG-E2").await.unwrap().is_none());

    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-E".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-E3", 1_700_000_060),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "MSG-E").await.unwrap().unwrap();
    assert!(msg.revoked);
    assert!(msg.text.is_none());
    assert!(msg.message.is_none());
}

#[tokio::test]
async fn reactions_add_replace_and_remove() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let alice = "559900000002@s.whatsapp.net";

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("target"),
            incoming_info(GROUP, PEER, "MSG-R", 1_700_000_000),
        )],
    )
    .await;

    let react = |emoji: &str, id: &str, ts: i64| {
        message_event(
            wa::Message {
                reaction_message: MessageField::some(wa::message::ReactionMessage {
                    key: MessageField::some(wa::MessageKey {
                        id: Some("MSG-R".into()),
                        ..Default::default()
                    }),
                    text: Some(emoji.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            incoming_info(GROUP, alice, id, ts),
        )
    };

    feed(&chat_store, [react("👍", "R1", 1_700_000_010)]).await;
    let reactions = chat_store.reactions(&chat, "MSG-R").await.unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");
    assert_eq!(reactions[0].sender_jid, jid(alice));

    // Same sender replaces their reaction (PK upsert), doesn't add a second.
    feed(&chat_store, [react("❤️", "R2", 1_700_000_020)]).await;
    let reactions = chat_store.reactions(&chat, "MSG-R").await.unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "❤️");

    // Empty text removes it.
    feed(&chat_store, [react("", "R3", 1_700_000_030)]).await;
    assert!(
        chat_store
            .reactions(&chat, "MSG-R")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn local_edit_updates_own_message_and_preview() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-EDIT",
            &wa::Message::text("typo"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_edit(
            &chat,
            "OUT-EDIT",
            &wa::Message::text("fixed"),
            Utc.timestamp_opt(1_700_000_050, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let msg = chat_store
        .message(&chat, "OUT-EDIT")
        .await
        .unwrap()
        .unwrap();
    assert!(msg.from_me);
    assert_eq!(msg.text.as_deref(), Some("fixed"));
    assert_eq!(
        msg.message
            .as_deref()
            .and_then(|message| message.conversation.as_deref()),
        Some("fixed")
    );
    assert_eq!(
        msg.edited_at.map(|timestamp| timestamp.timestamp()),
        Some(1_700_000_050)
    );
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("fixed"));

    // The local API keeps the event path's monotonic edit semantics.
    chat_store
        .record_edit(
            &chat,
            "OUT-EDIT",
            &wa::Message::text("stale"),
            Utc.timestamp_opt(1_700_000_025, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let msg = chat_store
        .message(&chat, "OUT-EDIT")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.text.as_deref(), Some("fixed"));
}

#[tokio::test]
async fn local_revoke_tombstones_own_message_and_absorbs_edits() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-REVOKE",
            &wa::Message::text("delete me"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_revoke(
            &chat,
            "OUT-REVOKE",
            Utc.timestamp_opt(1_700_000_050, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let msg = chat_store
        .message(&chat, "OUT-REVOKE")
        .await
        .unwrap()
        .unwrap();
    assert!(msg.revoked);
    assert!(msg.text.is_none());
    assert!(msg.message.is_none());
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert!(chats[0].last_message_preview.is_none());

    chat_store
        .record_edit(
            &chat,
            "OUT-REVOKE",
            &wa::Message::text("resurrected"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let msg = chat_store
        .message(&chat, "OUT-REVOKE")
        .await
        .unwrap()
        .unwrap();
    assert!(msg.revoked);
    assert!(msg.text.is_none());
}

#[tokio::test]
async fn local_reaction_adds_replaces_and_removes_own_reaction() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let target = wa::MessageKey {
        remote_jid: Some(GROUP.into()),
        from_me: Some(false),
        id: Some("MSG-LOCAL-REACTION".into()),
        participant: Some(PEER.into()),
    };

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("target"),
            incoming_info(GROUP, PEER, "MSG-LOCAL-REACTION", 1_700_000_000),
        )],
    )
    .await;

    chat_store
        .record_reaction(
            &chat,
            &target,
            "👍",
            Utc.timestamp_opt(1_700_000_020, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let reactions = chat_store
        .reactions(&chat, "MSG-LOCAL-REACTION")
        .await
        .unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");
    assert_eq!(reactions[0].sender_jid, Jid::default());

    // A stale local mirror cannot replace the latest reaction.
    chat_store
        .record_reaction(
            &chat,
            &target,
            "❤️",
            Utc.timestamp_opt(1_700_000_010, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let reactions = chat_store
        .reactions(&chat, "MSG-LOCAL-REACTION")
        .await
        .unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");

    chat_store
        .record_reaction(
            &chat,
            &target,
            "❤️",
            Utc.timestamp_opt(1_700_000_030, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_reaction(
            &chat,
            &target,
            "",
            Utc.timestamp_opt(1_700_000_040, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    assert!(
        chat_store
            .reactions(&chat, "MSG-LOCAL-REACTION")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn local_reaction_requires_a_target_id() {
    let (_store, chat_store) = test_store().await;
    let err = chat_store
        .record_reaction(
            &jid(PEER),
            &wa::MessageKey::default(),
            "👍",
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .expect_err("missing target id must fail");
    assert!(err.to_string().contains("storage error"));
}

#[tokio::test]
async fn local_reaction_checks_the_target_author_on_id_collision() {
    let (store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let mallory = "559900000066@s.whatsapp.net";

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("surviving target"),
                incoming_info(GROUP, PEER, "REACTION-COLLISION", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("colliding target"),
                incoming_info(GROUP, mallory, "REACTION-COLLISION", 1_700_000_010),
            ),
        ],
    )
    .await;

    let target_for = |participant: &str| wa::MessageKey {
        remote_jid: Some(GROUP.into()),
        from_me: Some(false),
        id: Some("REACTION-COLLISION".into()),
        participant: Some(participant.into()),
    };
    chat_store
        .record_reaction(
            &chat,
            &target_for(mallory),
            "👎",
            Utc.timestamp_opt(1_700_000_020, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    assert!(
        chat_store
            .reactions(&chat, "REACTION-COLLISION")
            .await
            .unwrap()
            .is_empty(),
        "a key for the colliding author must not attach to the surviving target"
    );

    // Device suffixes and the peer's mapped PN/LID alias do not change the
    // participant's author identity.
    add_lid_mapping(&store).await;
    chat_store
        .record_reaction(
            &chat,
            &target_for("111000011112222:48@lid"),
            "👍",
            Utc.timestamp_opt(1_700_000_030, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let reactions = chat_store
        .reactions(&chat, "REACTION-COLLISION")
        .await
        .unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");
}

#[tokio::test]
async fn local_reaction_removal_blocks_stale_history_reaction() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let target = wa::MessageKey {
        remote_jid: Some(GROUP.into()),
        from_me: Some(false),
        id: Some("MSG-REACTION-TOMBSTONE".into()),
        participant: Some(PEER.into()),
    };

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("target"),
            incoming_info(GROUP, PEER, "MSG-REACTION-TOMBSTONE", 1_700_000_000),
        )],
    )
    .await;
    chat_store
        .record_reaction(
            &chat,
            &target,
            "👍",
            Utc.timestamp_opt(1_700_000_010, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_reaction(
            &chat,
            &target,
            "",
            Utc.timestamp_opt(1_700_000_020, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let history = wa::HistorySync {
        sync_type: wa::history_sync::HistorySyncType::RECENT,
        conversations: vec![wa::Conversation {
            id: GROUP.to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: MessageField::some(wa::WebMessageInfo {
                    key: MessageField::some(target.clone()),
                    reactions: vec![wa::Reaction {
                        key: MessageField::some(wa::MessageKey {
                            from_me: Some(true),
                            ..target.clone()
                        }),
                        text: Some("👍".into()),
                        sender_timestamp_ms: Some(1_700_000_010_000),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    feed(&chat_store, [history_sync_event(history)]).await;
    assert!(
        chat_store
            .reactions(&chat, "MSG-REACTION-TOMBSTONE")
            .await
            .unwrap()
            .is_empty(),
        "stale history must not resurrect a removed reaction"
    );

    // A genuinely newer reaction still replaces the hidden tombstone.
    chat_store
        .record_reaction(
            &chat,
            &target,
            "❤️",
            Utc.timestamp_opt(1_700_000_030, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let reactions = chat_store
        .reactions(&chat, "MSG-REACTION-TOMBSTONE")
        .await
        .unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "❤️");
}

#[tokio::test]
async fn local_amendments_do_not_mutate_a_colliding_peer_message() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("peer content"),
            incoming_info(GROUP, PEER, "COLLIDING-ID", 1_700_000_000),
        )],
    )
    .await;
    chat_store
        .record_outgoing(
            &chat,
            "COLLIDING-ID",
            &wa::Message::text("own colliding content"),
            Utc.timestamp_opt(1_700_000_010, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_edit(
            &chat,
            "COLLIDING-ID",
            &wa::Message::text("own edit"),
            Utc.timestamp_opt(1_700_000_020, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_revoke(
            &chat,
            "COLLIDING-ID",
            Utc.timestamp_opt(1_700_000_030, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let msg = chat_store
        .message(&chat, "COLLIDING-ID")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.sender_jid, jid(PEER));
    assert!(!msg.from_me);
    assert_eq!(msg.text.as_deref(), Some("peer content"));
    assert!(!msg.revoked);
}

#[tokio::test]
async fn keyset_pagination_covers_all_pages_in_order() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    let events: Vec<Event> = (0..5)
        .map(|i| {
            message_event(
                wa::Message::text(format!("m{i}")),
                incoming_info(PEER, PEER, &format!("MSG-{i}"), 1_700_000_000 + i),
            )
        })
        .collect();
    feed(&chat_store, events).await;

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = chat_store.messages(&chat, cursor.take(), 2).await.unwrap();
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 2);
        cursor = page.last().map(Into::into);
        seen.extend(page.into_iter().map(|m| m.text.unwrap()));
    }
    // Newest first, no duplicates, no gaps.
    assert_eq!(seen, ["m4", "m3", "m2", "m1", "m0"]);
}

#[tokio::test]
async fn history_sync_materializes_without_clobbering_live_rows() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // A live copy arrives first (e.g. offline drain beat the history chunk).
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("live copy"),
            incoming_info(PEER, PEER, "MSG-H1", 1_700_000_000),
        )],
    )
    .await;

    let make_wmi = |id: &str, from_me: bool, ts: u64, text: &str| wa::WebMessageInfo {
        key: MessageField::some(wa::MessageKey {
            remote_jid: Some(PEER.into()),
            from_me: Some(from_me),
            id: Some(id.into()),
            ..Default::default()
        }),
        message: MessageField::from_box(Box::new(wa::Message::text(text))),
        message_timestamp: Some(ts),
        status: Some(wa::web_message_info::Status::READ),
        push_name: Some("Alice Example".into()),
        ..Default::default()
    };
    let history = wa::HistorySync {
        sync_type: wa::history_sync::HistorySyncType::RECENT,
        conversations: vec![
            // Fresh chat (no live row): mute/pin land via the INSERT path.
            // Wire values are unix seconds; the store must convert to ms.
            wa::Conversation {
                id: "559900000004@s.whatsapp.net".to_string(),
                conversation_timestamp: Some(1_700_000_500),
                mute_end_time: Some(1_800_000_000),
                pinned: Some(1_700_000_800),
                ..Default::default()
            },
            wa::Conversation {
                id: PEER.to_string(),
                name: Some("Alice".into()),
                conversation_timestamp: Some(1_700_000_900),
                unread_count: Some(0),
                messages: vec![
                    wa::HistorySyncMsg {
                        message: MessageField::some(make_wmi(
                            "MSG-H1",
                            false,
                            1_700_000_000,
                            "stale history copy",
                        )),
                        ..Default::default()
                    },
                    wa::HistorySyncMsg {
                        message: MessageField::some(make_wmi(
                            "MSG-H2",
                            true,
                            1_700_000_900,
                            "sent",
                        )),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ],
        pushnames: vec![wa::Pushname {
            id: Some("559900000003@s.whatsapp.net".into()),
            pushname: Some("Bob Example".into()),
        }],
        ..Default::default()
    };

    feed(&chat_store, [history_sync_event(history)]).await;

    // Chat identity from history; live message content preserved.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 2);
    let alice = chats
        .iter()
        .find(|c| c.jid == jid(PEER))
        .expect("alice chat");
    assert_eq!(alice.name.as_deref(), Some("Alice"));
    // History backfills the denormalized preview (newest materialized row).
    assert_eq!(alice.last_message_preview.as_deref(), Some("sent"));
    assert_eq!(alice.last_message_kind, Some(MessageKind::Text));
    // Seconds-to-ms conversion: a future mute/pin must not decode as 1970.
    let muted = chats
        .iter()
        .find(|c| c.jid == jid("559900000004@s.whatsapp.net"))
        .expect("muted chat");
    assert!(muted.muted_until.unwrap().year() > 2020);
    assert!(muted.pinned_at.unwrap().year() > 2020);

    let live = chat_store.message(&chat, "MSG-H1").await.unwrap().unwrap();
    assert_eq!(live.text.as_deref(), Some("live copy"));

    let hist = chat_store.message(&chat, "MSG-H2").await.unwrap().unwrap();
    assert!(hist.from_me);
    assert_eq!(hist.text.as_deref(), Some("sent"));
    assert_eq!(hist.status, MessageStatus::Read);

    // Pushnames from the remainder landed.
    let bob = chat_store
        .contact(&jid("559900000003@s.whatsapp.net"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bob.push_name.as_deref(), Some("Bob Example"));
}

#[tokio::test]
async fn undecryptable_placeholder_is_replaced_by_recovery() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    let info = incoming_info(PEER, PEER, "MSG-U", 1_700_000_000);
    feed(
        &chat_store,
        [Event::UndecryptableMessage(
            wacore::types::events::UndecryptableMessage::builder()
                .info(Arc::new(info.clone()))
                .is_unavailable(false)
                .unavailable_type(wacore::types::events::UnavailableType::Unknown)
                .decrypt_fail_mode(wacore::types::events::DecryptFailMode::Show)
                .build(),
        )],
    )
    .await;
    let placeholder = chat_store.message(&chat, "MSG-U").await.unwrap().unwrap();
    assert_eq!(placeholder.kind, MessageKind::Undecryptable);
    assert!(placeholder.message.is_none());

    // PDO/retry later recovers the real content under the same id.
    feed(
        &chat_store,
        [message_event(wa::Message::text("recovered"), info)],
    )
    .await;
    let recovered = chat_store.message(&chat, "MSG-U").await.unwrap().unwrap();
    assert_eq!(recovered.kind, MessageKind::Text);
    assert_eq!(recovered.text.as_deref(), Some("recovered"));
}

#[tokio::test]
async fn mark_chat_as_read_resets_unread_count() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("a"),
                incoming_info(PEER, PEER, "MSG-A", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("b"),
                incoming_info(PEER, PEER, "MSG-B", 1_700_000_001),
            ),
        ],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 2);
    assert_eq!(chat_store.unread_total().await.unwrap(), 2);

    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_100, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    ..Default::default()
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 0);
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);
}

#[tokio::test]
async fn invalidation_broadcast_fires_per_batch() {
    let (_store, chat_store) = test_store().await;
    let mut changes = chat_store.subscribe();

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("ping"),
            incoming_info(PEER, PEER, "MSG-N", 1_700_000_000),
        )],
    )
    .await;

    let mut got_chats = false;
    let mut got_messages = false;
    // Both signals were sent before flush() returned; drain with a timeout so
    // a regression fails fast instead of hanging.
    for _ in 0..3 {
        match tokio::time::timeout(Duration::from_secs(5), changes.recv()).await {
            Ok(Ok(StoreChange::Chats)) => got_chats = true,
            Ok(Ok(StoreChange::Messages { chat })) => {
                assert_eq!(chat, jid(PEER));
                got_messages = true;
            }
            Ok(Ok(StoreChange::Contacts)) => {}
            Ok(Err(_)) | Err(_) => break,
        }
        if got_chats && got_messages {
            break;
        }
    }
    assert!(got_chats && got_messages);
}

#[tokio::test]
async fn group_receipts_track_per_user_state() {
    let (_store, chat_store) = test_store().await;
    let group = jid(GROUP);
    let alice = "559900000002@s.whatsapp.net";

    chat_store
        .record_outgoing(
            &group,
            "OUT-G",
            &wa::Message::text("hey group"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                // Production receipts leave is_group defaulted (false); the
                // store must derive groupness from the chat JID.
                .source(MessageSource {
                    chat: group.clone(),
                    sender: jid(alice),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-G".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_010, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )],
    )
    .await;

    let receipts = chat_store.receipts(&group, "OUT-G").await.unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].user_jid, jid(alice));
    assert_eq!(receipts[0].status, MessageStatus::Read);
}

#[cfg(feature = "search")]
#[tokio::test]
async fn full_text_search_finds_and_survives_operator_input() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("reunião amanhã às dez"),
                incoming_info(PEER, PEER, "MSG-S1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("outra coisa qualquer"),
                incoming_info(PEER, PEER, "MSG-S2", 1_700_000_001),
            ),
        ],
    )
    .await;

    let hits = chat_store.search_messages("reunião", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "MSG-S1");

    // Prefix match on partial words.
    let hits = chat_store.search_messages("aman", 10).await.unwrap();
    assert_eq!(hits.len(), 1);

    // FTS5 operator characters must not produce a syntax error.
    let hits = chat_store
        .search_messages("reunião AND NOT (\"", 10)
        .await
        .unwrap();
    assert!(hits.len() <= 1);

    // Edited text re-indexes.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-S2".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("agora relevante"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-S3", 1_700_000_002),
        )],
    )
    .await;
    let hits = chat_store.search_messages("relevante", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "MSG-S2");
    assert!(
        chat_store
            .search_messages("outra", 10)
            .await
            .unwrap()
            .is_empty()
    );

    // NULL transitions must keep the index sound: revoke clears text
    // (text -> NULL) and a recovered placeholder gains text (NULL -> text).
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-S1".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-S4", 1_700_000_003),
        )],
    )
    .await;
    assert!(
        chat_store
            .search_messages("reunião", 10)
            .await
            .unwrap()
            .is_empty()
    );

    let info = incoming_info(PEER, PEER, "MSG-S5", 1_700_000_004);
    feed(
        &chat_store,
        [Event::UndecryptableMessage(
            wacore::types::events::UndecryptableMessage::builder()
                .info(Arc::new(info.clone()))
                .is_unavailable(false)
                .unavailable_type(wacore::types::events::UnavailableType::Unknown)
                .decrypt_fail_mode(wacore::types::events::DecryptFailMode::Show)
                .build(),
        )],
    )
    .await;
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("conteúdo recuperado"),
            info,
        )],
    )
    .await;
    let hits = chat_store.search_messages("recuperado", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "MSG-S5");
}

#[tokio::test]
async fn revoke_before_content_is_not_resurrected() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Offline drain can deliver the revoke before the content it targets.
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-RB".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-RB2", 1_700_000_010),
        )],
    )
    .await;
    let tombstone = chat_store.message(&chat, "MSG-RB").await.unwrap().unwrap();
    assert!(tombstone.revoked);

    // The content arriving later (redelivery path, overwrite=true) must not
    // un-revoke the tombstone.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("too late"),
            incoming_info(PEER, PEER, "MSG-RB", 1_700_000_000),
        )],
    )
    .await;
    let still_revoked = chat_store.message(&chat, "MSG-RB").await.unwrap().unwrap();
    assert!(still_revoked.revoked);
    assert!(still_revoked.text.is_none());
    // ...and the skipped redelivery must not surface its content in the
    // chat-list preview either.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert!(
        chats
            .iter()
            .all(|c| c.last_message_preview.as_deref() != Some("too late"))
    );
}

#[tokio::test]
async fn edit_of_revoked_message_is_a_no_op() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("original"),
            incoming_info(PEER, PEER, "MSG-ER", 1_700_000_000),
        )],
    )
    .await;
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-ER".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-ER2", 1_700_000_010),
        )],
    )
    .await;

    // An edit targeting the tombstone must not resurrect content.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-ER".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("resurrected"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-ER3", 1_700_000_020),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "MSG-ER").await.unwrap().unwrap();
    assert!(msg.revoked);
    assert!(msg.text.is_none());
    assert!(msg.message.is_none());
}

#[tokio::test]
async fn same_millisecond_sibling_does_not_hijack_preview() {
    let (_store, chat_store) = test_store().await;

    // Two messages in the same millisecond; (timestamp, msg_id) ordering makes
    // MSG-Z2 the latest.
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("first"),
                incoming_info(PEER, PEER, "MSG-A1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("second"),
                incoming_info(PEER, PEER, "MSG-Z2", 1_700_000_000),
            ),
        ],
    )
    .await;

    // Editing the OLDER same-millisecond sibling must not steal the preview.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-A1".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("hijacked"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-E1", 1_700_000_100),
        )],
    )
    .await;
    let msg = chat_store
        .message(&jid(PEER), "MSG-A1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.text.as_deref(), Some("hijacked"));
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("second"));
}

#[tokio::test]
async fn pdo_recovery_does_not_double_count_unread() {
    let (_store, chat_store) = test_store().await;

    let info = incoming_info(PEER, PEER, "MSG-DC", 1_700_000_000);
    feed(
        &chat_store,
        [Event::UndecryptableMessage(
            wacore::types::events::UndecryptableMessage::builder()
                .info(Arc::new(info.clone()))
                .is_unavailable(false)
                .unavailable_type(wacore::types::events::UnavailableType::Unknown)
                .decrypt_fail_mode(wacore::types::events::DecryptFailMode::Show)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);

    // PDO recovery replaces the placeholder under the same id: same message,
    // must not count twice.
    feed(
        &chat_store,
        [message_event(wa::Message::text("recovered"), info)],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn edit_of_latest_message_refreshes_preview_and_stale_edit_is_ignored() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("original"),
            incoming_info(PEER, PEER, "MSG-EP", 1_700_000_000),
        )],
    )
    .await;

    let edit_with = |text: &str, id: &str, ts: i64| {
        message_event(
            wa::Message {
                protocol_message: MessageField::some(wa::message::ProtocolMessage {
                    key: MessageField::some(wa::MessageKey {
                        id: Some("MSG-EP".into()),
                        ..Default::default()
                    }),
                    r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
                    edited_message: MessageField::from_box(Box::new(wa::Message::text(text))),
                    ..Default::default()
                }),
                ..Default::default()
            },
            incoming_info(PEER, PEER, id, ts),
        )
    };

    feed(&chat_store, [edit_with("edited", "E1", 1_700_000_100)]).await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("edited"));

    // A stale edit (older than the applied one) must not roll content back.
    feed(&chat_store, [edit_with("stale", "E2", 1_700_000_050)]).await;
    let msg = chat_store.message(&chat, "MSG-EP").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("edited"));
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("edited"));
}

#[tokio::test]
async fn delete_for_me_cleans_satellites_and_recomputes_preview() {
    let (_store, chat_store) = test_store().await;
    let group = jid(GROUP);
    let alice = "559900000002@s.whatsapp.net";

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("keep me"),
            incoming_info(GROUP, PEER, "MSG-K", 1_700_000_000),
        )],
    )
    .await;
    chat_store
        .record_outgoing(
            &group,
            "MSG-D",
            &wa::Message::text("delete me"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: group.clone(),
                    sender: jid(alice),
                    is_group: true,
                    ..Default::default()
                })
                .message_ids(vec!["MSG-D".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_110, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.receipts(&group, "MSG-D").await.unwrap().len(), 1);

    feed(
        &chat_store,
        [Event::DeleteMessageForMeUpdate(
            wacore::types::events::DeleteMessageForMeUpdate::builder()
                .chat_jid(group.clone())
                .message_id("MSG-D".to_string())
                .from_me(true)
                .timestamp(Utc.timestamp_opt(1_700_000_200, 0).unwrap())
                .action(Box::new(
                    wa::sync_action_value::DeleteMessageForMeAction::default(),
                ))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    assert!(chat_store.message(&group, "MSG-D").await.unwrap().is_none());
    assert!(
        chat_store
            .receipts(&group, "MSG-D")
            .await
            .unwrap()
            .is_empty()
    );
    // The chat-list preview falls back to the newest remaining message.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("keep me"));
}

#[tokio::test]
async fn stale_reaction_timestamp_does_not_replace_newer() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("target"),
            incoming_info(PEER, PEER, "MSG-RT", 1_700_000_000),
        )],
    )
    .await;
    let react = |emoji: &str, id: &str, ts: i64| {
        message_event(
            wa::Message {
                reaction_message: MessageField::some(wa::message::ReactionMessage {
                    key: MessageField::some(wa::MessageKey {
                        id: Some("MSG-RT".into()),
                        ..Default::default()
                    }),
                    text: Some(emoji.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            incoming_info(PEER, PEER, id, ts),
        )
    };
    feed(&chat_store, [react("👍", "R1", 1_700_000_200)]).await;
    // An older copy (e.g. replayed from a history chunk) must not win.
    feed(&chat_store, [react("❤️", "R2", 1_700_000_100)]).await;
    let reactions = chat_store.reactions(&chat, "MSG-RT").await.unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");

    // Neither must a stale REMOVE delete it...
    feed(&chat_store, [react("", "R3", 1_700_000_150)]).await;
    let reactions = chat_store.reactions(&chat, "MSG-RT").await.unwrap();
    assert_eq!(reactions.len(), 1);

    // ...while a newer remove still works.
    feed(&chat_store, [react("", "R4", 1_700_000_300)]).await;
    assert!(
        chat_store
            .reactions(&chat, "MSG-RT")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn flush_surfaces_a_failed_batch() {
    let (store, chat_store) = test_store().await;

    // Sabotage the schema so the next batch rolls back.
    store
        .shared()
        .run(|conn| {
            diesel::sql_query("ALTER TABLE messages RENAME TO messages_gone")
                .execute(conn)
                .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .unwrap();

    let handler = chat_store.handler();
    handler.handle_event(Arc::new(message_event(
        wa::Message::text("will fail"),
        incoming_info(PEER, PEER, "MSG-F", 1_700_000_000),
    )));
    let err = chat_store.flush().await.expect_err("batch must fail");
    assert!(matches!(
        err,
        whatsapp_rust_chat_store::ChatStoreError::WriteBatchFailed(_)
    ));

    // Restore and confirm the writer survived the failure.
    store
        .shared()
        .run(|conn| {
            diesel::sql_query("ALTER TABLE messages_gone RENAME TO messages")
                .execute(conn)
                .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .unwrap();
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("works again"),
            incoming_info(PEER, PEER, "MSG-OK", 1_700_000_010),
        )],
    )
    .await;
    assert!(
        chat_store
            .message(&jid(PEER), "MSG-OK")
            .await
            .unwrap()
            .is_some()
    );
}

#[cfg(feature = "search")]
#[tokio::test]
async fn fts_backfills_rows_that_predate_the_index() {
    let (store, chat_store) = test_store().await;

    // Simulate a database created before the `search` feature existed: drop
    // the FTS objects, then write rows with no triggers in place.
    store
        .shared()
        .run(|conn| {
            for stmt in [
                "DROP TRIGGER IF EXISTS messages_fts_ai",
                "DROP TRIGGER IF EXISTS messages_fts_ad",
                "DROP TRIGGER IF EXISTS messages_fts_au",
                "DROP TABLE IF EXISTS messages_fts",
            ] {
                diesel::sql_query(stmt)
                    .execute(conn)
                    .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            }
            Ok(())
        })
        .await
        .unwrap();
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("mensagem antiga indexável"),
            incoming_info(PEER, PEER, "MSG-BF", 1_700_000_000),
        )],
    )
    .await;

    // A second open on the same file recreates the index and must backfill it.
    let chat_store2 = ChatStore::new(&store).await.unwrap();
    let hits = chat_store2.search_messages("antiga", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "MSG-BF");
}

#[tokio::test]
async fn forever_mute_is_not_reported_as_unmuted() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("hi"),
                incoming_info(PEER, PEER, "MSG-M", 1_700_000_000),
            ),
            Event::MuteUpdate(
                wacore::types::events::MuteUpdate::builder()
                    .jid(jid(PEER))
                    .timestamp(Utc.timestamp_opt(1_700_000_100, 0).unwrap())
                    // muted with no end timestamp = muted forever
                    .action(Box::new(wa::sync_action_value::MuteAction {
                        muted: Some(true),
                        ..Default::default()
                    }))
                    .from_full_sync(false)
                    .build(),
            ),
        ],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    let muted_until = chats[0].muted_until.expect("forever mute must be Some");
    assert!(muted_until > wacore::time::now_utc());
    assert_eq!(muted_until, chrono::DateTime::<Utc>::MAX_UTC);
}

#[tokio::test]
async fn clear_chat_reflects_surviving_starred_messages() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("starred survivor"),
                incoming_info(PEER, PEER, "MSG-S1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("cleared away"),
                incoming_info(PEER, PEER, "MSG-S2", 1_700_000_100),
            ),
            Event::StarUpdate(
                wacore::types::events::StarUpdate::builder()
                    .chat_jid(chat.clone())
                    .message_id("MSG-S1".to_string())
                    .from_me(false)
                    .timestamp(Utc.timestamp_opt(1_700_000_200, 0).unwrap())
                    .action(Box::new(wa::sync_action_value::StarAction {
                        starred: Some(true),
                    }))
                    .from_full_sync(false)
                    .build(),
            ),
            Event::ClearChatUpdate(
                wacore::types::events::ClearChatUpdate::builder()
                    .jid(chat.clone())
                    .delete_starred(false)
                    .delete_media(false)
                    .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                    .action(Box::new(wa::sync_action_value::ClearChatAction::default()))
                    .from_full_sync(false)
                    .build(),
            ),
        ],
    )
    .await;

    // The starred message survives and becomes the preview (not a blank one
    // with the deleted message's stale kind).
    assert!(chat_store.message(&chat, "MSG-S1").await.unwrap().is_some());
    assert!(chat_store.message(&chat, "MSG-S2").await.unwrap().is_none());
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(
        chats[0].last_message_preview.as_deref(),
        Some("starred survivor")
    );
    assert_eq!(chats[0].last_message_kind, Some(MessageKind::Text));
    assert_eq!(chats[0].unread_count, 0);
}

fn range_up_to(ts_secs: i64) -> MessageField<wa::sync_action_value::SyncActionMessageRange> {
    MessageField::some(wa::sync_action_value::SyncActionMessageRange {
        last_message_timestamp: Some(ts_secs),
        ..Default::default()
    })
}

#[tokio::test]
async fn mark_read_range_preserves_newer_unread() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("covered"),
                incoming_info(PEER, PEER, "MSG-C1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("newer than the replayed read"),
                incoming_info(PEER, PEER, "MSG-C2", 1_700_000_100),
            ),
        ],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 2);

    // A delayed mark-read whose range ends at the first message must not
    // swallow the second one's unread state.
    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn ranged_clear_and_delete_keep_newer_messages() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Ranged clear: only rows up to the boundary go away.
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("old"),
                incoming_info(PEER, PEER, "MSG-O", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("newer than the action"),
                incoming_info(PEER, PEER, "MSG-N", 1_700_000_100),
            ),
        ],
    )
    .await;
    feed(
        &chat_store,
        [Event::ClearChatUpdate(
            wacore::types::events::ClearChatUpdate::builder()
                .jid(chat.clone())
                .delete_starred(true)
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .action(Box::new(wa::sync_action_value::ClearChatAction {
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert!(chat_store.message(&chat, "MSG-O").await.unwrap().is_none());
    assert!(chat_store.message(&chat, "MSG-N").await.unwrap().is_some());
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(
        chats[0].last_message_preview.as_deref(),
        Some("newer than the action")
    );

    // Ranged delete-chat: newer rows keep the chat alive.
    feed(
        &chat_store,
        [Event::DeleteChatUpdate(
            wacore::types::events::DeleteChatUpdate::builder()
                .jid(chat.clone())
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_060, 0).unwrap())
                .action(Box::new(wa::sync_action_value::DeleteChatAction {
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert!(chat_store.message(&chat, "MSG-N").await.unwrap().is_some());
    assert_eq!(chat_store.chats(false, 10).await.unwrap().len(), 1);

    // Unranged delete-chat: everything goes.
    feed(
        &chat_store,
        [Event::DeleteChatUpdate(
            wacore::types::events::DeleteChatUpdate::builder()
                .jid(chat.clone())
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_070, 0).unwrap())
                .action(Box::new(wa::sync_action_value::DeleteChatAction::default()))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert!(chat_store.chats(false, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn revoke_tombstone_keeps_target_from_me() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Revoke of OUR OWN message (key.fromMe = true) arriving before the
    // content: the tombstone must not read as incoming forever.
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-FM".into()),
                from_me: Some(true),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-FM2", 1_700_000_000),
        )],
    )
    .await;
    let tombstone = chat_store.message(&chat, "MSG-FM").await.unwrap().unwrap();
    assert!(tombstone.revoked);
    assert!(tombstone.from_me);
}

#[tokio::test]
async fn recompute_does_not_resurrect_tombstone_kind() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("older"),
                incoming_info(PEER, PEER, "MSG-T1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("newest, will be revoked"),
                incoming_info(PEER, PEER, "MSG-T2", 1_700_000_100),
            ),
        ],
    )
    .await;
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-T2".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-T3", 1_700_000_200),
        )],
    )
    .await;

    // Deleting the OLDER row forces a recompute whose newest row is the
    // tombstone: neither its text (None already) nor its pre-revoke kind may
    // come back.
    feed(
        &chat_store,
        [Event::DeleteMessageForMeUpdate(
            wacore::types::events::DeleteMessageForMeUpdate::builder()
                .chat_jid(chat.clone())
                .message_id("MSG-T1".to_string())
                .from_me(false)
                .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                .action(Box::new(
                    wa::sync_action_value::DeleteMessageForMeAction::default(),
                ))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert!(chats[0].last_message_preview.is_none());
    assert!(chats[0].last_message_kind.is_none());
}

#[tokio::test]
async fn same_millisecond_preview_follows_arrival_not_id() {
    let (_store, chat_store) = test_store().await;

    // Two rows on the same millisecond, applied in an order that reverses
    // their msg_id order. The preview belongs to the one that arrived last —
    // the id sorts higher but says nothing about time.
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("higher id, arrived first"),
                incoming_info(PEER, PEER, "MSG-Z9", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("lower id, arrived last"),
                incoming_info(PEER, PEER, "MSG-A1", 1_700_000_000),
            ),
        ],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(
        chats[0].last_message_preview.as_deref(),
        Some("lower id, arrived last")
    );
}

/// Arrival only breaks ties. A genuinely older message materialized late
/// (offline drain, history backfill) still must not take the preview.
#[tokio::test]
async fn late_but_older_message_does_not_hijack_preview() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("newest"),
                incoming_info(PEER, PEER, "MSG-NEW", 1_700_000_100),
            ),
            message_event(
                wa::Message::text("older, applied later"),
                incoming_info(PEER, PEER, "MSG-OLD", 1_700_000_000),
            ),
        ],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("newest"));
}

#[tokio::test]
async fn range_boundary_covers_the_whole_wire_second() {
    let (_store, chat_store) = test_store().await;

    // 500 ms into the boundary second: the wire range (whole seconds) covers
    // it, so a mark-read up to that second must clear it.
    let mut info = incoming_info(PEER, PEER, "MSG-SUB", 1_700_000_000);
    info.timestamp = Utc.timestamp_opt(1_700_000_000, 500_000_000).unwrap();
    feed(
        &chat_store,
        [message_event(wa::Message::text("sub-second"), info)],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);

    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_001, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);
}

#[tokio::test]
async fn keyed_range_spares_unlisted_same_second_siblings() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Two messages inside the SAME wire second.
    for (id, text) in [("MSG-IN", "covered"), ("MSG-OUT", "not in the range")] {
        feed(
            &chat_store,
            [message_event(
                wa::Message::text(text),
                incoming_info(PEER, PEER, id, 1_700_000_000),
            )],
        )
        .await;
    }

    // The action enumerates only MSG-IN at the boundary.
    let range = MessageField::some(wa::sync_action_value::SyncActionMessageRange {
        last_message_timestamp: Some(1_700_000_000),
        messages: vec![wa::sync_action_value::SyncActionMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-IN".into()),
                remote_jid: Some(PEER.into()),
                ..Default::default()
            }),
            timestamp: Some(1_700_000_000),
        }],
        ..Default::default()
    });
    feed(
        &chat_store,
        [Event::ClearChatUpdate(
            wacore::types::events::ClearChatUpdate::builder()
                .jid(chat.clone())
                .delete_starred(true)
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_001, 0).unwrap())
                .action(Box::new(wa::sync_action_value::ClearChatAction {
                    message_range: range,
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    // Only the enumerated sibling went away; the other survives, still unread.
    assert!(chat_store.message(&chat, "MSG-IN").await.unwrap().is_none());
    assert!(
        chat_store
            .message(&chat, "MSG-OUT")
            .await
            .unwrap()
            .is_some()
    );
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 1);
    assert_eq!(
        chats[0].last_message_preview.as_deref(),
        Some("not in the range")
    );
}

#[tokio::test]
async fn ranged_clear_keeps_unread_survivors_counted() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("cleared"),
                incoming_info(PEER, PEER, "MSG-CL", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("unread survivor"),
                incoming_info(PEER, PEER, "MSG-UN", 1_700_000_100),
            ),
        ],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 2);

    feed(
        &chat_store,
        [Event::ClearChatUpdate(
            wacore::types::events::ClearChatUpdate::builder()
                .jid(chat.clone())
                .delete_starred(true)
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .action(Box::new(wa::sync_action_value::ClearChatAction {
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    // The survivor is still there AND still counted as unread.
    assert!(chat_store.message(&chat, "MSG-UN").await.unwrap().is_some());
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn delayed_read_self_keeps_newer_unread() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("read on the phone"),
                incoming_info(PEER, PEER, "MSG-RS1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("arrived after the read"),
                incoming_info(PEER, PEER, "MSG-RS2", 1_700_000_100),
            ),
        ],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 2);

    // Offline-delayed read-self covering only the FIRST message: the second
    // one keeps its badge.
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-RS1".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(true)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn read_self_spares_unlisted_same_timestamp_siblings() {
    let (_store, chat_store) = test_store().await;

    // Two incoming rows at the SAME stored timestamp; the receipt names one.
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("named"),
                incoming_info(PEER, PEER, "MSG-RSA", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("same instant, not named"),
                incoming_info(PEER, PEER, "MSG-RSB", 1_700_000_000),
            ),
        ],
    )
    .await;
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-RSA".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn stale_read_self_does_not_reinflate_the_badge() {
    let (_store, chat_store) = test_store().await;

    let read_self = |ids: Vec<&str>, ts: i64| {
        Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(ids.into_iter().map(String::from).collect())
                .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(true)
                .build(),
        )
    };

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("first"),
                incoming_info(PEER, PEER, "MSG-B1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("second"),
                incoming_info(PEER, PEER, "MSG-B2", 1_700_000_100),
            ),
        ],
    )
    .await;

    // Newest receipt clears everything...
    feed(
        &chat_store,
        [read_self(vec!["MSG-B1", "MSG-B2"], 1_700_000_150)],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // ...and a stale replay covering only the FIRST message must not
    // resurrect the badge for the second.
    feed(&chat_store, [read_self(vec!["MSG-B1"], 1_700_000_050)]).await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);
}

#[tokio::test]
async fn cross_sender_id_reuse_cannot_rewrite_a_message() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let mallory = "559900000066@s.whatsapp.net";

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("victim's original words"),
            incoming_info(GROUP, PEER, "MSG-VIC", 1_700_000_000),
        )],
    )
    .await;

    // Message ids are sender-chosen: a different participant reusing the id
    // must be deduped, never rewrite the victim's row.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("attacker rewrite"),
            incoming_info(GROUP, mallory, "MSG-VIC", 1_700_000_100),
        )],
    )
    .await;

    let msg = chat_store.message(&chat, "MSG-VIC").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("victim's original words"));
    assert_eq!(msg.sender_jid, jid(PEER));
}

#[tokio::test]
async fn stale_ranged_mark_read_respects_the_read_cursor() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("first"),
                incoming_info(PEER, PEER, "MSG-RC1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("second"),
                incoming_info(PEER, PEER, "MSG-RC2", 1_700_000_100),
            ),
        ],
    )
    .await;

    // A read-self covering everything clears the badge and advances the cursor.
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-RC1".to_string(), "MSG-RC2".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_150, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // A STALE ranged mark-read (covers only the first message) replays later:
    // it must not resurrect the second message's badge.
    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    message_range: range_up_to(1_700_000_000),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);
}

#[tokio::test]
async fn keyed_mark_read_spares_unlisted_same_second_sibling() {
    let (_store, chat_store) = test_store().await;

    // Two incoming rows in the SAME wire second; the mark-read names only one.
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("named"),
                incoming_info(PEER, PEER, "MSG-KA", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("unnamed sibling"),
                incoming_info(PEER, PEER, "MSG-KB", 1_700_000_000),
            ),
        ],
    )
    .await;

    let range = MessageField::some(wa::sync_action_value::SyncActionMessageRange {
        last_message_timestamp: Some(1_700_000_000),
        messages: vec![wa::sync_action_value::SyncActionMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-KA".into()),
                remote_jid: Some(PEER.into()),
                ..Default::default()
            }),
            timestamp: Some(1_700_000_000),
        }],
        ..Default::default()
    });
    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_001, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    message_range: range,
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn late_materialized_old_message_does_not_badge_after_read() {
    let (_store, chat_store) = test_store().await;

    // Unranged mark-read on an EMPTY chat: the cursor must advance off the
    // action's own timestamp.
    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_100, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    ..Default::default()
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    // An OLDER message materializes afterwards (offline drain): already read,
    // must not badge.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("late but old"),
            incoming_info(PEER, PEER, "MSG-LATE", 1_700_000_000),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // While a genuinely NEW message still badges.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("genuinely new"),
            incoming_info(PEER, PEER, "MSG-NEW", 1_700_000_200),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn admin_revoke_tombstone_keeps_target_author() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(GROUP);
    let admin = "559900000077@s.whatsapp.net";
    let author = "559900000088@s.whatsapp.net";

    // Admin revoke arriving BEFORE the original: the tombstone must attribute
    // the message to its author (revoke key participant), not to the admin.
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-ADM".into()),
                from_me: Some(false),
                participant: Some(author.into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(GROUP, admin, "MSG-ADM2", 1_700_000_000),
        )],
    )
    .await;
    let tombstone = chat_store.message(&chat, "MSG-ADM").await.unwrap().unwrap();
    assert!(tombstone.revoked);
    assert_eq!(tombstone.sender_jid, jid(author));
}

#[tokio::test]
async fn redelivery_after_edit_keeps_edited_content() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    let original = || {
        message_event(
            wa::Message::text("original"),
            incoming_info(PEER, PEER, "MSG-RED", 1_700_000_000),
        )
    };
    feed(&chat_store, [original()]).await;
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-RED".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("edited"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-RED2", 1_700_000_100),
        )],
    )
    .await;

    // A duplicate delivery of the PRE-edit original must not roll content back.
    feed(&chat_store, [original()]).await;
    let msg = chat_store.message(&chat, "MSG-RED").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("edited"));
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("edited"));
}

#[tokio::test]
async fn late_same_instant_sibling_still_badges_after_read_self() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("named"),
            incoming_info(PEER, PEER, "MSG-SI1", 1_700_000_000),
        )],
    )
    .await;
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-SI1".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // An unlisted sibling at the SAME instant materializes later (offline
    // drain): the receipt didn't cover it, so it must badge.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("same instant, uncovered"),
            incoming_info(PEER, PEER, "MSG-SI2", 1_700_000_000),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn delete_for_me_drops_the_victims_badge() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("stays unread"),
                incoming_info(PEER, PEER, "MSG-U1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("deleted while unread"),
                incoming_info(PEER, PEER, "MSG-U2", 1_700_000_100),
            ),
        ],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 2);

    feed(
        &chat_store,
        [Event::DeleteMessageForMeUpdate(
            wacore::types::events::DeleteMessageForMeUpdate::builder()
                .chat_jid(chat.clone())
                .message_id("MSG-U2".to_string())
                .from_me(false)
                .timestamp(Utc.timestamp_opt(1_700_000_200, 0).unwrap())
                .action(Box::new(
                    wa::sync_action_value::DeleteMessageForMeAction::default(),
                ))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn keyed_read_covers_a_message_that_materializes_later() {
    let (_store, chat_store) = test_store().await;

    // The keyed mark-read arrives BEFORE the message it names (read on
    // another device, local drain lagging).
    let range = MessageField::some(wa::sync_action_value::SyncActionMessageRange {
        last_message_timestamp: Some(1_700_000_000),
        messages: vec![wa::sync_action_value::SyncActionMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-FUT".into()),
                remote_jid: Some(PEER.into()),
                ..Default::default()
            }),
            timestamp: Some(1_700_000_000),
        }],
        ..Default::default()
    });
    feed(
        &chat_store,
        [Event::MarkChatAsReadUpdate(
            wacore::types::events::MarkChatAsReadUpdate::builder()
                .jid(jid(PEER))
                .timestamp(Utc.timestamp_opt(1_700_000_001, 0).unwrap())
                .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(true),
                    message_range: range,
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;

    // The named message materializes afterwards: covered, no badge...
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("read elsewhere before arriving"),
            incoming_info(PEER, PEER, "MSG-FUT", 1_700_000_000),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // ...while an unnamed same-second sibling still badges.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("uncovered sibling"),
            incoming_info(PEER, PEER, "MSG-SIB", 1_700_000_000),
        )],
    )
    .await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 1);
}

#[tokio::test]
async fn wire_indefinite_mute_value_reads_as_forever() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("hi"),
                incoming_info(PEER, PEER, "MSG-IM", 1_700_000_000),
            ),
            Event::MuteUpdate(
                wacore::types::events::MuteUpdate::builder()
                    .jid(jid(PEER))
                    .timestamp(Utc.timestamp_opt(1_700_000_100, 0).unwrap())
                    // The wire's indefinite-mute sentinel (what the library's
                    // own mute_chat() sends).
                    .action(Box::new(wa::sync_action_value::MuteAction {
                        muted: Some(true),
                        mute_end_timestamp: Some(-1),
                        ..Default::default()
                    }))
                    .from_full_sync(false)
                    .build(),
            ),
        ],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].muted_until, Some(chrono::DateTime::<Utc>::MAX_UTC));
}

#[tokio::test]
async fn early_tombstone_materializes_and_badges_the_chat() {
    let (_store, chat_store) = test_store().await;

    // A revoke for a message we never saw, in a chat we never saw: the chat
    // must still appear (the deleted message DID happen) and badge.
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-GHOST".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            revoke,
            incoming_info(PEER, PEER, "MSG-GHOST2", 1_700_000_000),
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
    assert!(chats[0].last_message_at.is_some());
    assert!(chats[0].last_message_preview.is_none());
    assert_eq!(chats[0].unread_count, 1);
}

fn mark_read_event(chat: &str, read: bool, ts_secs: i64) -> Event {
    Event::MarkChatAsReadUpdate(
        wacore::types::events::MarkChatAsReadUpdate::builder()
            .jid(jid(chat))
            .timestamp(Utc.timestamp_opt(ts_secs, 0).unwrap())
            .action(Box::new(wa::sync_action_value::MarkChatAsReadAction {
                read: Some(read),
                ..Default::default()
            }))
            .from_full_sync(false)
            .build(),
    )
}

#[tokio::test]
async fn noop_mark_read_clears_manual_unread_marker() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("oi"),
            incoming_info(PEER, PEER, "MSG-MU", 1_700_000_000),
        )],
    )
    .await;
    feed(&chat_store, [mark_read_event(PEER, true, 1_700_000_010)]).await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    // Manually mark unread, then read the chat again: the cursor can't move
    // (nothing new arrived), but the marker must still clear.
    feed(&chat_store, [mark_read_event(PEER, false, 1_700_000_020)]).await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, -1);

    feed(&chat_store, [mark_read_event(PEER, true, 1_700_000_030)]).await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 0);
}

#[tokio::test]
async fn noop_read_self_clears_manual_unread_marker() {
    let (_store, chat_store) = test_store().await;

    let read_self = |ts: i64| {
        Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER),
                    sender: jid(PEER),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-RS".to_string()])
                .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(false)
                .build(),
        )
    };

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("oi"),
            incoming_info(PEER, PEER, "MSG-RS", 1_700_000_000),
        )],
    )
    .await;
    feed(&chat_store, [read_self(1_700_000_010)]).await;
    assert_eq!(chat_store.unread_total().await.unwrap(), 0);

    feed(&chat_store, [mark_read_event(PEER, false, 1_700_000_020)]).await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, -1);

    // Re-reading on the phone re-sends the same boundary: a no-op for the
    // cursor, but the marker must still clear.
    feed(&chat_store, [read_self(1_700_000_030)]).await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 0);
}

#[tokio::test]
async fn edit_before_target_materializes_edited_content() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Offline drain reordering: the edit is applied before the original.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-EB".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("fixed"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            edit,
            incoming_info(PEER, PEER, "MSG-EB2", 1_700_000_050),
        )],
    )
    .await;

    // The edited content materializes up front and badges like the original.
    let msg = chat_store.message(&chat, "MSG-EB").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("fixed"));
    assert!(msg.edited_at.is_some());
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("fixed"));
    assert_eq!(chats[0].unread_count, 1);

    // The original's late arrival must neither restore pre-edit text nor
    // count the same message twice.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("typo"),
            incoming_info(PEER, PEER, "MSG-EB", 1_700_000_000),
        )],
    )
    .await;
    let msg = chat_store.message(&chat, "MSG-EB").await.unwrap().unwrap();
    assert_eq!(msg.text.as_deref(), Some("fixed"));
    assert!(msg.edited_at.is_some());
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("fixed"));
    assert_eq!(chats[0].unread_count, 1);
}

#[tokio::test]
async fn server_nack_marks_outgoing_failed() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-NACK",
            &wa::Message::text("oi"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-NACK".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .error("479".to_string())
                .build(),
        )],
    )
    .await;
    let msg = chat_store
        .message(&chat, "OUT-NACK")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Error);

    // A stray nack must not regress a message a peer already received.
    chat_store
        .record_outgoing(
            &chat,
            "OUT-READ",
            &wa::Message::text("oi2"),
            Utc.timestamp_opt(1_700_000_200, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    feed(
        &chat_store,
        [
            Event::Receipt(
                Receipt::builder()
                    .source(MessageSource {
                        chat: chat.clone(),
                        sender: chat.clone(),
                        ..Default::default()
                    })
                    .message_ids(vec!["OUT-READ".to_string()])
                    .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                    .r#type(ReceiptType::Read)
                    .offline(false)
                    .build(),
            ),
            Event::ServerAck(
                ServerAck::builder()
                    .id("OUT-READ".to_string())
                    .class("message".to_string())
                    .from(chat.clone())
                    .error("479".to_string())
                    .build(),
            ),
        ],
    )
    .await;
    let msg = chat_store
        .message(&chat, "OUT-READ")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);

    // ...nor one the server already accepted: the positive ack answered the
    // stanza, so a later nack for the same id is noise.
    chat_store
        .record_outgoing(
            &chat,
            "OUT-ACKED",
            &wa::Message::text("oi3"),
            Utc.timestamp_opt(1_700_000_400, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    feed(
        &chat_store,
        [
            Event::ServerAck(
                ServerAck::builder()
                    .id("OUT-ACKED".to_string())
                    .class("message".to_string())
                    .from(chat.clone())
                    .build(),
            ),
            Event::ServerAck(
                ServerAck::builder()
                    .id("OUT-ACKED".to_string())
                    .class("message".to_string())
                    .from(chat.clone())
                    .error("479".to_string())
                    .build(),
            ),
        ],
    )
    .await;
    let msg = chat_store
        .message(&chat, "OUT-ACKED")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::ServerAck);
}

// ── LID/PN identity resolution (issue #1078) ────────────────────────────────

const PEER_LID: &str = "111000011112222@lid";

/// Learn PEER <-> PEER_LID in the device store's mapping table, the alias
/// index the chat-store resolves against.
async fn add_lid_mapping(store: &SqliteStore) {
    use wacore::store::traits::{LidPnMappingEntry, ProtocolStore};
    // The mapping table's FK needs the device row the client normally creates.
    store.create_new_device().await.expect("create device");
    store
        .put_lid_mapping(&LidPnMappingEntry {
            lid: "111000011112222".into(),
            phone_number: "559900000001".into(),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            learning_source: "usync".into(),
        })
        .await
        .expect("put mapping");
}

fn read_receipt(chat: &str, ids: Vec<&str>, ts: i64) -> Event {
    Event::Receipt(
        Receipt::builder()
            .source(MessageSource {
                chat: jid(chat),
                sender: jid(chat),
                ..Default::default()
            })
            .message_ids(ids.into_iter().map(String::from).collect())
            .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
            .r#type(ReceiptType::Read)
            .offline(false)
            .build(),
    )
}

/// The issue #1078 scenario: rows stored under the phone-number key before
/// any mapping was known, delivered/read receipts arriving LID-keyed.
#[tokio::test]
async fn lid_receipt_advances_pn_keyed_rows() {
    let (store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-SPLIT",
            &wa::Message::text("oi"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-SPLIT".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .build(),
        )],
    )
    .await;

    add_lid_mapping(&store).await;
    feed(
        &chat_store,
        [read_receipt(PEER_LID, vec!["OUT-SPLIT"], 1_700_000_200)],
    )
    .await;

    let msg = chat_store
        .message(&chat, "OUT-SPLIT")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
    // No stray @lid twin was created by the receipt.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, chat);
    // Either identity reads the same thread.
    let via_lid = chat_store
        .message(&jid(PEER_LID), "OUT-SPLIT")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(via_lid.status, MessageStatus::Read);
}

/// The mirror direction: LID-keyed rows, PN-keyed receipt.
#[tokio::test]
async fn pn_receipt_advances_lid_keyed_rows() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-L",
            &wa::Message::text("oi"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    add_lid_mapping(&store).await;
    feed(
        &chat_store,
        [read_receipt(PEER, vec!["OUT-L"], 1_700_000_200)],
    )
    .await;

    let msg = chat_store
        .message(&jid(PEER_LID), "OUT-L")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// Without a mapping the receipt still can't be attributed (the pre-fix
/// behavior); once the mapping is learned, a replayed receipt heals the row.
#[tokio::test]
async fn receipt_heals_once_mapping_is_learned() {
    let (store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-H",
            &wa::Message::text("oi"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [read_receipt(PEER_LID, vec!["OUT-H"], 1_700_000_200)],
    )
    .await;
    let msg = chat_store.message(&chat, "OUT-H").await.unwrap().unwrap();
    assert_eq!(msg.status, MessageStatus::Pending);

    add_lid_mapping(&store).await;
    feed(
        &chat_store,
        [read_receipt(PEER_LID, vec!["OUT-H"], 1_700_000_300)],
    )
    .await;
    let msg = chat_store.message(&chat, "OUT-H").await.unwrap().unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// With the mapping known up front, a brand-new chat is keyed by the LID (WA
/// Web `selectChatForOneOnOneMessage`) even when addressed by phone number,
/// so later LID-keyed receipts hit exactly.
#[tokio::test]
async fn known_mapping_keys_new_chat_by_lid() {
    let (store, chat_store) = test_store().await;
    add_lid_mapping(&store).await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-NEW",
            &wa::Message::text("primeira"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    feed(
        &chat_store,
        [read_receipt(PEER_LID, vec!["OUT-NEW"], 1_700_000_200)],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER_LID));
    // The embedder still addresses (and reads) by phone number.
    let msg = chat_store
        .message(&jid(PEER), "OUT-NEW")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// An inbound LID-addressed message joins the peer's existing PN-keyed
/// thread instead of opening a twin chat.
#[tokio::test]
async fn inbound_lid_message_joins_existing_pn_thread() {
    let (store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("antes"),
            incoming_info(PEER, PEER, "MSG-P1", 1_700_000_000),
        )],
    )
    .await;
    add_lid_mapping(&store).await;
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("depois"),
            incoming_info(PEER_LID, PEER_LID, "MSG-L1", 1_700_000_100),
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
    assert_eq!(chats[0].unread_count, 2);
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("depois"));
    let messages = chat_store.messages(&jid(PEER_LID), None, 10).await.unwrap();
    assert_eq!(messages.len(), 2);
}

/// A read-self receipt arriving LID-keyed clears the PN-keyed thread's badge
/// instead of materializing an empty @lid chat row.
#[tokio::test]
async fn lid_read_self_clears_pn_thread_badge() {
    let (store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("um"),
                incoming_info(PEER, PEER, "MSG-RS-A", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("dois"),
                incoming_info(PEER, PEER, "MSG-RS-B", 1_700_000_010),
            ),
        ],
    )
    .await;
    add_lid_mapping(&store).await;
    feed(
        &chat_store,
        [Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(PEER_LID),
                    sender: jid(PEER_LID),
                    ..Default::default()
                })
                .message_ids(vec!["MSG-RS-A".to_string(), "MSG-RS-B".to_string()])
                .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                .r#type(ReceiptType::ReadSelf)
                .offline(false)
                .build(),
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
    assert_eq!(chats[0].unread_count, 0);
}

/// Splits left behind by the pre-fix behavior merge on demand: messages fold
/// into the newer-activity side, the badge is recounted, sticky prefs
/// survive, and the repair is idempotent.
#[tokio::test]
async fn reconcile_merges_split_pair() {
    let (store, chat_store) = test_store().await;

    // No mapping yet: two independent chats form (the split).
    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("via pn"),
                incoming_info(PEER, PEER, "MSG-SP-A", 1_700_000_000),
            ),
            Event::PinUpdate(
                wacore::types::events::PinUpdate::builder()
                    .jid(jid(PEER))
                    .timestamp(Utc.timestamp_opt(1_700_000_050, 0).unwrap())
                    .action(Box::new(wa::sync_action_value::PinAction {
                        pinned: Some(true),
                    }))
                    .from_full_sync(false)
                    .build(),
            ),
            message_event(
                wa::Message::text("via lid"),
                incoming_info(PEER_LID, PEER_LID, "MSG-SP-B", 1_700_000_100),
            ),
        ],
    )
    .await;
    assert_eq!(chat_store.chats(false, 10).await.unwrap().len(), 2);

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    // Newest activity was on the LID side, so that key survives.
    assert_eq!(chats[0].jid, jid(PEER_LID));
    assert_eq!(chats[0].unread_count, 2);
    assert!(chats[0].pinned_at.is_some());
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("via lid"));
    let messages = chat_store.messages(&jid(PEER), None, 10).await.unwrap();
    assert_eq!(messages.len(), 2);

    // Idempotent.
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].unread_count, 2);
}

/// The same message stored under both keys keeps the most advanced status
/// after the merge.
#[tokio::test]
async fn merge_advances_duplicate_row_status() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-DUP",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-DUP",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    // Only the LID copy saw the read receipt.
    feed(
        &chat_store,
        [read_receipt(PEER_LID, vec!["OUT-DUP"], 1_700_000_200)],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    // PN side had the newer activity, so it is the surviving key…
    assert_eq!(chats[0].jid, jid(PEER));
    // …and the duplicate's Read status survived onto it.
    let msg = chat_store
        .message(&jid(PEER), "OUT-DUP")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// Both identities recorded the same state before the mapping reconciled. The
/// merge direction is decided by chat activity, which says nothing about which
/// side saw the receipt first, so the earlier instant has to be carried over
/// rather than left to whichever key wins.
#[tokio::test]
async fn merge_keeps_the_earlier_instant_for_a_state_both_sides_recorded() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-TS",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-TS",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    // One sender, addressing the thread by each of its identities in turn —
    // the shape of a PN/LID transition, and the only way the same
    // (message, user, state) lands under both chat keys. The LID side saw the
    // read first; the PN side, which newer activity will make the merge
    // destination, saw it later.
    let read_from_lid_addressed_to = |chat: &str, ts: i64| {
        Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(chat),
                    sender: jid(PEER_LID),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-TS".to_string()])
                .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )
    };
    feed(
        &chat_store,
        [
            read_from_lid_addressed_to(PEER_LID, 1_700_000_200),
            read_from_lid_addressed_to(PEER, 1_700_000_800),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let receipts = chat_store.receipts(&jid(PEER), "OUT-TS").await.unwrap();
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert_eq!(
        receipts[0].timestamp.timestamp(),
        1_700_000_200,
        "the losing side saw it first, so its instant is the true one"
    );
}

/// One peer, not two. A 1:1's receipt names whoever the peer sent from, so a
/// thread that changed identity mid-flight accumulates rows under both — and
/// the merge is the only place that can put them back together, since moving
/// `chat_jid` leaves `user_jid` untouched.
#[tokio::test]
async fn merge_folds_a_split_peer_identity_into_one_user() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-WHO",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-WHO",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    // Delivered reported from the LID identity, read from the PN one.
    feed(
        &chat_store,
        [
            peer_receipt(
                jid(PEER_LID),
                vec!["OUT-WHO"],
                ReceiptType::Delivered,
                1_700_000_200,
            ),
            peer_receipt(jid(PEER), vec!["OUT-WHO"], ReceiptType::Read, 1_700_000_300),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let receipts = chat_store.receipts(&jid(PEER), "OUT-WHO").await.unwrap();
    let mut users: Vec<String> = receipts.iter().map(|r| r.user_jid.to_string()).collect();
    users.dedup();
    assert_eq!(
        users,
        vec![PEER],
        "both states belong to one peer after the merge: {receipts:?}"
    );
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.status, r.timestamp.timestamp()))
            .collect::<Vec<_>>(),
        vec![
            (MessageStatus::Delivered, 1_700_000_200),
            (MessageStatus::Read, 1_700_000_300),
        ],
        "and both states survive: {receipts:?}"
    );
}

/// The collision the identity rewrite cannot resolve by itself: both
/// identities recorded the *same* state, and one of the rows already sits
/// under the surviving key. Renaming it would duplicate the row that is
/// already there, so it is skipped — and the `chat_jid = src` sweep never
/// reaches it, because it was never filed under `src`.
#[tokio::test]
async fn merge_drops_the_twin_left_by_a_same_state_collision() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-TWIN",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-TWIN",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let read_from = |sender: &str, chat: &str, ts: i64| {
        Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(chat),
                    sender: jid(sender),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-TWIN".to_string()])
                .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )
    };
    // Same state, same surviving chat, two identities — and the retiring
    // identity is the one that saw it first.
    feed(
        &chat_store,
        [
            read_from(PEER_LID, PEER, 1_700_000_200),
            read_from(PEER, PEER, 1_700_000_800),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let receipts = chat_store.receipts(&jid(PEER), "OUT-TWIN").await.unwrap();
    assert_eq!(
        receipts.len(),
        1,
        "the skipped twin must not outlive the merge: {receipts:?}"
    );
    assert_eq!(receipts[0].user_jid, jid(PEER));
    assert_eq!(
        receipts[0].timestamp.timestamp(),
        1_700_000_200,
        "and it leaves its instant behind"
    );
}

/// The same collision from the retiring side. Here the skipped row sits under
/// `src`, so the dedup sweep must reach it before the chat rename does —
/// otherwise the rename carries it to the surviving thread untouched, still
/// naming the identity being retired.
#[tokio::test]
async fn merge_drops_a_same_state_collision_left_on_the_retiring_side() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-SRC-TWIN",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store
        .record_outgoing(
            &jid(PEER_LID),
            "OUT-SRC-TWIN",
            &wa::Message::text("dup"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let read_from = |sender: &str, chat: &str, ts: i64| {
        Event::Receipt(
            Receipt::builder()
                .source(MessageSource {
                    chat: jid(chat),
                    sender: jid(sender),
                    ..Default::default()
                })
                .message_ids(vec!["OUT-SRC-TWIN".to_string()])
                .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                .r#type(ReceiptType::Read)
                .offline(false)
                .build(),
        )
    };
    // Both identities record the same state under the LID chat — the side that
    // newer PN activity will retire.
    feed(
        &chat_store,
        [
            read_from(PEER_LID, PEER_LID, 1_700_000_800),
            read_from(PEER, PEER_LID, 1_700_000_200),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let receipts = chat_store
        .receipts(&jid(PEER), "OUT-SRC-TWIN")
        .await
        .unwrap();
    assert_eq!(
        receipts.len(),
        1,
        "the collision survivor must not ride the rename over: {receipts:?}"
    );
    assert_eq!(receipts[0].user_jid, jid(PEER));
    assert_eq!(
        receipts[0].timestamp.timestamp(),
        1_700_000_200,
        "and the earlier instant survives"
    );
}

/// A reaction addressed by the peer's other identity lands on the stored
/// message (routing picks the existing thread).
#[tokio::test]
async fn lid_reaction_reaches_pn_keyed_message() {
    let (store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("alvo"),
            incoming_info(PEER, PEER, "MSG-R1", 1_700_000_000),
        )],
    )
    .await;
    add_lid_mapping(&store).await;

    let reaction = wa::Message {
        reaction_message: MessageField::some(wa::message::ReactionMessage {
            key: MessageField::some(wa::MessageKey {
                remote_jid: Some(PEER_LID.to_string()),
                from_me: Some(false),
                id: Some("MSG-R1".to_string()),
                ..Default::default()
            }),
            text: Some("👍".to_string()),
            sender_timestamp_ms: Some(1_700_000_100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [message_event(
            reaction,
            incoming_info(PEER_LID, PEER_LID, "MSG-R1-REACT", 1_700_000_100),
        )],
    )
    .await;

    let reactions = chat_store.reactions(&jid(PEER), "MSG-R1").await.unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");
    // No twin chat was opened for the reaction.
    assert_eq!(chat_store.chats(false, 10).await.unwrap().len(), 1);
}

/// An edit or revoke that reached only one side of a split survives the
/// merge: the source copy's newer edit content and tombstone fold into the
/// surviving row (same monotonic rules as the live path).
#[tokio::test]
async fn merge_folds_src_side_edit_and_revoke() {
    let (store, chat_store) = test_store().await;

    // No mapping: duplicate copies of both messages under each identity.
    for chat in [PEER, PEER_LID] {
        feed(
            &chat_store,
            [
                message_event(
                    wa::Message::text("typo"),
                    incoming_info(chat, chat, "MSG-ED", 1_700_000_000),
                ),
                message_event(
                    wa::Message::text("apaga"),
                    incoming_info(chat, chat, "MSG-RV", 1_700_000_010),
                ),
            ],
        )
        .await;
    }
    // Edit and revoke land only on the LID side.
    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-ED".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text("consertada"))),
            ..Default::default()
        }),
        ..Default::default()
    };
    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("MSG-RV".into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [
            message_event(
                edit,
                incoming_info(PEER_LID, PEER_LID, "MSG-ED2", 1_700_000_050),
            ),
            message_event(
                revoke,
                incoming_info(PEER_LID, PEER_LID, "MSG-RV2", 1_700_000_060),
            ),
            // Newer activity on the PN side makes it the merge destination.
            message_event(
                wa::Message::text("mais nova"),
                incoming_info(PEER, PEER, "MSG-NW", 1_700_000_100),
            ),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
    let edited = chat_store
        .message(&jid(PEER), "MSG-ED")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(edited.text.as_deref(), Some("consertada"));
    assert!(edited.edited_at.is_some());
    let revoked = chat_store
        .message(&jid(PEER), "MSG-RV")
        .await
        .unwrap()
        .unwrap();
    assert!(revoked.revoked);
    assert!(revoked.text.is_none());
}

/// Competing edits on both copies of the same message: the strictly newer
/// edit wins the merge in either direction.
#[tokio::test]
async fn merge_keeps_strictly_newer_edit_across_sides() {
    let (store, chat_store) = test_store().await;

    // No mapping: duplicate copies under each identity, then both sides edit.
    for chat in [PEER, PEER_LID] {
        feed(
            &chat_store,
            [
                message_event(
                    wa::Message::text("v0-a"),
                    incoming_info(chat, chat, "MSG-CE1", 1_700_000_000),
                ),
                message_event(
                    wa::Message::text("v0-b"),
                    incoming_info(chat, chat, "MSG-CE2", 1_700_000_010),
                ),
            ],
        )
        .await;
    }
    let edit = |target: &str, text: &str| wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some(target.into()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::from_box(Box::new(wa::Message::text(text))),
            ..Default::default()
        }),
        ..Default::default()
    };
    feed(
        &chat_store,
        [
            // CE1: the PN (destination) side carries the newer edit.
            message_event(
                edit("MSG-CE1", "lid antiga"),
                incoming_info(PEER_LID, PEER_LID, "MSG-CE1-EL", 1_700_000_050),
            ),
            message_event(
                edit("MSG-CE1", "pn mais nova"),
                incoming_info(PEER, PEER, "MSG-CE1-EP", 1_700_000_080),
            ),
            // CE2: the LID (source) side carries the newer edit.
            message_event(
                edit("MSG-CE2", "pn antiga"),
                incoming_info(PEER, PEER, "MSG-CE2-EP", 1_700_000_050),
            ),
            message_event(
                edit("MSG-CE2", "lid mais nova"),
                incoming_info(PEER_LID, PEER_LID, "MSG-CE2-EL", 1_700_000_080),
            ),
            // Newest activity keeps the PN side as merge destination.
            message_event(
                wa::Message::text("mais nova"),
                incoming_info(PEER, PEER, "MSG-CE-NW", 1_700_000_100),
            ),
        ],
    )
    .await;

    add_lid_mapping(&store).await;
    chat_store.reconcile_chat(&jid(PEER)).unwrap();
    chat_store.flush().await.unwrap();

    let ce1 = chat_store
        .message(&jid(PEER), "MSG-CE1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ce1.text.as_deref(), Some("pn mais nova"));
    assert_eq!(
        ce1.edited_at.map(|t| t.timestamp()),
        Some(1_700_000_080),
        "destination's newer edit must not be clobbered by the source's older one"
    );
    let ce2 = chat_store
        .message(&jid(PEER), "MSG-CE2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ce2.text.as_deref(), Some("lid mais nova"));
    assert_eq!(ce2.edited_at.map(|t| t.timestamp()), Some(1_700_000_080));
}

// ── Companion-device (device-suffixed) identities (issue #1095) ─────────────

/// A peer's linked device as the binary decoder yields it: `user:48@lid`,
/// carrying the LID domain-type byte in `agent`.
fn companion(user: &str, device: u16) -> Jid {
    Jid {
        user: user.into(),
        server: wacore_binary::Server::Lid,
        agent: 1,
        device,
        integrator: 0,
    }
}

fn peer_receipt(source: Jid, ids: Vec<&str>, ty: ReceiptType, ts: i64) -> Event {
    Event::Receipt(
        Receipt::builder()
            .source(MessageSource {
                chat: source.clone(),
                sender: source,
                ..Default::default()
            })
            .message_ids(ids.into_iter().map(String::from).collect())
            .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
            .r#type(ty)
            .offline(false)
            .build(),
    )
}

/// A fresh LID-only thread has no counterpart to fall back to, so the direct
/// match has to succeed on the normalized key.
#[tokio::test]
async fn companion_device_receipt_advances_status() {
    let (_store, chat_store) = test_store().await;
    let chat = jid("10203040506070@lid");

    chat_store
        .record_outgoing(
            &chat,
            "OUT-AD-1",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    feed(
        &chat_store,
        [peer_receipt(
            companion("10203040506070", 48),
            vec!["OUT-AD-1"],
            ReceiptType::Delivered,
            1_700_000_200,
        )],
    )
    .await;

    let msg = chat_store
        .message(&chat, "OUT-AD-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Delivered);
    // The receipt keyed no thread of its own.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, chat);
}

/// Multi-device emits the read once, from whichever device read first. The
/// primary never re-sends it, so a companion read has to land.
#[tokio::test]
async fn companion_read_advances_past_primary_delivered() {
    let (_store, chat_store) = test_store().await;
    let chat = jid("10203040506070@lid");

    chat_store
        .record_outgoing(
            &chat,
            "OUT-AD-2",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    feed(
        &chat_store,
        [
            peer_receipt(
                chat.clone(),
                vec!["OUT-AD-2"],
                ReceiptType::Delivered,
                1_700_000_200,
            ),
            peer_receipt(
                companion("10203040506070", 48),
                vec!["OUT-AD-2"],
                ReceiptType::Read,
                1_700_000_300,
            ),
        ],
    )
    .await;

    let msg = chat_store
        .message(&chat, "OUT-AD-2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// Device-suffixed *and* addressed by the identity the rows are not keyed
/// under: normalization has to happen before the alternate-key retry, or the
/// mapping is never consulted.
#[tokio::test]
async fn companion_receipt_resolves_across_pn_lid_mapping() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-AD-3",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    add_lid_mapping(&store).await;

    feed(
        &chat_store,
        [peer_receipt(
            companion("111000011112222", 12),
            vec!["OUT-AD-3"],
            ReceiptType::Read,
            1_700_000_200,
        )],
    )
    .await;

    let msg = chat_store
        .message(&jid(PEER), "OUT-AD-3")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].jid, jid(PEER));
}

/// A 1:1 keeps its receipt rows, so a reader can say *when* the peer got and
/// read the message and not merely that they did. `messages.status` carries the
/// state it reached and no instant, which is the half WA Web's contact message
/// info renders as "Delivered hh:mm" above "Read hh:mm".
#[tokio::test]
async fn dm_receipts_record_when_each_state_was_reached() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    chat_store
        .record_outgoing(
            &peer,
            "OUT-DM-INFO",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    feed(
        &chat_store,
        [
            peer_receipt(
                peer.clone(),
                vec!["OUT-DM-INFO"],
                ReceiptType::Delivered,
                1_700_000_200,
            ),
            peer_receipt(
                peer.clone(),
                vec!["OUT-DM-INFO"],
                ReceiptType::Read,
                1_700_000_300,
            ),
        ],
    )
    .await;

    let receipts = chat_store.receipts(&peer, "OUT-DM-INFO").await.unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.user_jid.clone(), r.status, r.timestamp.timestamp()))
            .collect::<Vec<_>>(),
        vec![
            (peer.clone(), MessageStatus::Delivered, 1_700_000_200),
            (peer.clone(), MessageStatus::Read, 1_700_000_300),
        ],
        "both instants survive: {receipts:?}"
    );

    // The state on the message itself is unchanged by any of this.
    let msg = chat_store
        .message(&peer, "OUT-DM-INFO")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(msg.status, MessageStatus::Read);
}

/// A voice note's `played` is a third state, not a replacement for `read`.
#[tokio::test]
async fn dm_played_receipt_joins_read_rather_than_replacing_it() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    chat_store
        .record_outgoing(
            &peer,
            "OUT-DM-PTT",
            &wa::Message::text("ptt"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    for (ty, ts) in [
        (ReceiptType::Delivered, 1_700_000_200),
        (ReceiptType::Read, 1_700_000_300),
        (ReceiptType::Played, 1_700_000_400),
    ] {
        feed(
            &chat_store,
            [peer_receipt(peer.clone(), vec!["OUT-DM-PTT"], ty, ts)],
        )
        .await;
    }

    let receipts = chat_store.receipts(&peer, "OUT-DM-PTT").await.unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.status, r.timestamp.timestamp()))
            .collect::<Vec<_>>(),
        vec![
            (MessageStatus::Delivered, 1_700_000_200),
            (MessageStatus::Read, 1_700_000_300),
            (MessageStatus::Played, 1_700_000_400),
        ],
        "{receipts:?}"
    );
}

/// A replayed receipt is a duplicate, not a later event: the instant a state
/// was first reported is the one that stays.
#[tokio::test]
async fn a_replayed_dm_receipt_does_not_move_the_recorded_instant() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    chat_store
        .record_outgoing(
            &peer,
            "OUT-DM-DUP",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    for ts in [1_700_000_200, 1_700_000_900] {
        feed(
            &chat_store,
            [peer_receipt(
                peer.clone(),
                vec!["OUT-DM-DUP"],
                ReceiptType::Delivered,
                ts,
            )],
        )
        .await;
    }

    let receipts = chat_store.receipts(&peer, "OUT-DM-DUP").await.unwrap();
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert_eq!(receipts[0].timestamp.timestamp(), 1_700_000_200);
}

/// A receipt that only answers under the counterpart identity must file its
/// row there too. The satellite prune is per chat and collects receipt rows
/// whose message is absent from that chat, so a row left behind under the wire
/// key would not survive the next trim.
#[tokio::test]
async fn a_dm_receipt_resolved_by_alias_files_under_the_message_key() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-DM-ALIAS",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    add_lid_mapping(&store).await;

    // Addressed by LID while the row is keyed by PN.
    feed(
        &chat_store,
        [peer_receipt(
            jid(PEER_LID),
            vec!["OUT-DM-ALIAS"],
            ReceiptType::Read,
            1_700_000_200,
        )],
    )
    .await;

    // Reachable under either identity, since the reader resolves the alias.
    for addressed_as in [PEER, PEER_LID] {
        let receipts = chat_store
            .receipts(&jid(addressed_as), "OUT-DM-ALIAS")
            .await
            .unwrap();
        assert_eq!(receipts.len(), 1, "as {addressed_as}: {receipts:?}");
        assert_eq!(receipts[0].status, MessageStatus::Read);
        assert_eq!(receipts[0].timestamp.timestamp(), 1_700_000_200);
    }

    // Filed under the key the message actually lives at, not the wire key it
    // arrived addressed to — which is what keeps the per-chat satellite prune
    // from collecting it as an orphan.
    let stored: Vec<JidRow> = store
        .shared()
        .run(|conn| {
            diesel::sql_query(
                "SELECT chat_jid AS jid FROM message_receipts \
                 WHERE device_id = 1 AND msg_id = 'OUT-DM-ALIAS'",
            )
            .load(conn)
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))
        })
        .await
        .unwrap();
    assert_eq!(
        stored.iter().map(|r| r.jid.as_str()).collect::<Vec<_>>(),
        vec![PEER],
        "receipt follows the message's key, not the wire key"
    );
}

/// A receipt that advances nothing still has to file under the message's key.
/// The status not moving says the state was already reached, not that the row
/// lives somewhere else — so ownership cannot be read off the update count.
#[tokio::test]
async fn a_dm_receipt_behind_the_current_status_still_files_by_alias() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-DM-BEHIND",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    add_lid_mapping(&store).await;

    // Read first, then a Delivered that arrives late: it advances nothing,
    // because the message is already further along.
    feed(
        &chat_store,
        [
            peer_receipt(
                jid(PEER_LID),
                vec!["OUT-DM-BEHIND"],
                ReceiptType::Read,
                1_700_000_300,
            ),
            peer_receipt(
                jid(PEER_LID),
                vec!["OUT-DM-BEHIND"],
                ReceiptType::Delivered,
                1_700_000_200,
            ),
        ],
    )
    .await;

    let stored: Vec<JidRow> = store
        .shared()
        .run(|conn| {
            diesel::sql_query(
                "SELECT DISTINCT chat_jid AS jid FROM message_receipts \
                 WHERE device_id = 1 AND msg_id = 'OUT-DM-BEHIND'",
            )
            .load(conn)
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))
        })
        .await
        .unwrap();
    assert_eq!(
        stored.iter().map(|r| r.jid.as_str()).collect::<Vec<_>>(),
        vec![PEER],
        "the late receipt filed under the wire key instead of the message's"
    );

    let receipts = chat_store
        .receipts(&jid(PEER), "OUT-DM-BEHIND")
        .await
        .unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.status, r.timestamp.timestamp()))
            .collect::<Vec<_>>(),
        vec![
            (MessageStatus::Delivered, 1_700_000_200),
            (MessageStatus::Read, 1_700_000_300),
        ],
        "{receipts:?}"
    );
}

/// Receipts do not arrive in time order — an offline queue drains after the
/// live socket — so the state's instant is the earliest reported, not the
/// first one processed.
#[tokio::test]
async fn an_out_of_order_receipt_lowers_the_recorded_instant() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    chat_store
        .record_outgoing(
            &peer,
            "OUT-DM-ORDER",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    // The live device reports first, then a delayed report of the same state
    // that actually happened earlier.
    for ts in [1_700_000_900, 1_700_000_200] {
        feed(
            &chat_store,
            [peer_receipt(
                peer.clone(),
                vec!["OUT-DM-ORDER"],
                ReceiptType::Delivered,
                ts,
            )],
        )
        .await;
    }

    let receipts = chat_store.receipts(&peer, "OUT-DM-ORDER").await.unwrap();
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert_eq!(
        receipts[0].timestamp.timestamp(),
        1_700_000_200,
        "the earlier instant wins regardless of arrival order"
    );
}

/// A receipt naming a message no chat holds is dropped, not parked. The id is
/// the server's, and nothing here can tell an unrecorded send from a message
/// the user deleted — so parking one re-created metadata for messages that
/// were deliberately removed.
#[tokio::test]
async fn a_receipt_for_a_message_no_chat_holds_is_dropped() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    feed(
        &chat_store,
        [peer_receipt(
            peer.clone(),
            vec!["OUT-DM-UNKNOWN"],
            ReceiptType::Delivered,
            1_700_000_200,
        )],
    )
    .await;

    assert!(
        chat_store
            .receipts(&peer, "OUT-DM-UNKNOWN")
            .await
            .unwrap()
            .is_empty(),
        "nothing owns this id, so nothing is recorded for it"
    );
}

/// The case that motivates dropping: a delete removes a message and sweeps its
/// receipts, then a delayed or replayed receipt for it arrives. It must not
/// bring the deleted message's metadata back.
#[tokio::test]
async fn a_receipt_arriving_after_a_delete_does_not_resurrect_it() {
    let (_store, chat_store) = test_store().await;
    let peer = jid(PEER);

    chat_store
        .record_outgoing(
            &peer,
            "OUT-DM-GONE",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    feed(
        &chat_store,
        [peer_receipt(
            peer.clone(),
            vec!["OUT-DM-GONE"],
            ReceiptType::Delivered,
            1_700_000_200,
        )],
    )
    .await;
    assert_eq!(
        chat_store
            .receipts(&peer, "OUT-DM-GONE")
            .await
            .unwrap()
            .len(),
        1
    );

    feed(
        &chat_store,
        [Event::ClearChatUpdate(
            wacore::types::events::ClearChatUpdate::builder()
                .jid(peer.clone())
                .delete_starred(true)
                .delete_media(false)
                .timestamp(Utc.timestamp_opt(1_700_000_300, 0).unwrap())
                .action(Box::new(wa::sync_action_value::ClearChatAction {
                    message_range: None.into(),
                }))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    assert!(
        chat_store
            .message(&peer, "OUT-DM-GONE")
            .await
            .unwrap()
            .is_none()
    );

    // The peer's other device reports the same state, late.
    feed(
        &chat_store,
        [peer_receipt(
            peer.clone(),
            vec!["OUT-DM-GONE"],
            ReceiptType::Read,
            1_700_000_400,
        )],
    )
    .await;

    assert!(
        chat_store
            .receipts(&peer, "OUT-DM-GONE")
            .await
            .unwrap()
            .is_empty(),
        "a deleted message stays deleted, metadata and all"
    );
}

/// Dropping the unowned ones must not cost the aliased ones: a receipt
/// addressed by one identity for a message stored under the other still files
/// against the message.
#[tokio::test]
async fn an_aliased_receipt_still_files_against_its_message() {
    let (store, chat_store) = test_store().await;

    chat_store
        .record_outgoing(
            &jid(PEER),
            "OUT-DM-ALIASED",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    add_lid_mapping(&store).await;

    feed(
        &chat_store,
        [peer_receipt(
            jid(PEER_LID),
            vec!["OUT-DM-ALIASED"],
            ReceiptType::Delivered,
            1_700_000_200,
        )],
    )
    .await;

    let stored: Vec<JidRow> = store
        .shared()
        .run(|conn| {
            diesel::sql_query(
                "SELECT chat_jid AS jid FROM message_receipts \
                 WHERE device_id = 1 AND msg_id = 'OUT-DM-ALIASED'",
            )
            .load(conn)
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))
        })
        .await
        .unwrap();
    assert_eq!(
        stored.iter().map(|r| r.jid.as_str()).collect::<Vec<_>>(),
        vec![PEER],
        "filed where the message lives"
    );
}

/// A self receipt carrying a device must recount the real thread instead of
/// materializing a twin of it.
#[tokio::test]
async fn companion_read_self_recounts_the_real_chat() {
    let (_store, chat_store) = test_store().await;
    let chat = jid("10203040506070@lid");

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("oi"),
            incoming_info(
                "10203040506070@lid",
                "10203040506070@lid",
                "IN-AD-1",
                1_700_000_000,
            ),
        )],
    )
    .await;
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].unread_count, 1);

    feed(
        &chat_store,
        [peer_receipt(
            companion("10203040506070", 48),
            vec!["IN-AD-1"],
            ReceiptType::ReadSelf,
            1_700_000_100,
        )],
    )
    .await;

    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats.len(), 1, "no phantom chat: {chats:?}");
    assert_eq!(chats[0].jid, chat);
    assert_eq!(chats[0].unread_count, 0);
}

/// Messages keep the device on `sender` by design, so the push name of a peer
/// texting from WhatsApp Web has to be filed under the bare identity anyway.
#[tokio::test]
async fn companion_sender_push_name_lands_on_the_bare_contact() {
    let (_store, chat_store) = test_store().await;
    let bare = jid("10203040506070@lid");
    let device = companion("10203040506070", 48);

    let mut info = MessageInfo {
        source: MessageSource {
            chat: bare.clone(),
            sender: device.clone(),
            is_from_me: false,
            ..Default::default()
        },
        id: "IN-AD-2".to_string(),
        timestamp: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        ..Default::default()
    };
    info.push_name = "Alice Example".into();
    feed(&chat_store, [message_event(wa::Message::text("oi"), info)]).await;

    let contact = chat_store.contact(&bare).await.unwrap().unwrap();
    assert_eq!(contact.push_name.as_deref(), Some("Alice Example"));
    // And a caller holding the message's `sender` finds the same row.
    let via_device = chat_store.contact(&device).await.unwrap().unwrap();
    assert_eq!(via_device.jid, bare);
}

/// Receipts collapse by participant, not by device: a member reading on their
/// phone and on Web emits one receipt each, and both name the same person.
/// The two rows that survive are that person's two *states*, not two members.
#[tokio::test]
async fn group_receipts_from_two_devices_keep_one_participant() {
    let (_store, chat_store) = test_store().await;
    let group = jid(GROUP);

    chat_store
        .record_outgoing(
            &group,
            "OUT-G-AD",
            &wa::Message::text("olá"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    for (device, ty, ts) in [
        (0u16, ReceiptType::Delivered, 1_700_000_200),
        (48u16, ReceiptType::Read, 1_700_000_300),
    ] {
        feed(
            &chat_store,
            [Event::Receipt(
                Receipt::builder()
                    .source(MessageSource {
                        chat: group.clone(),
                        sender: companion("111000011112222", device),
                        ..Default::default()
                    })
                    .message_ids(vec!["OUT-G-AD".to_string()])
                    .timestamp(Utc.timestamp_opt(ts, 0).unwrap())
                    .r#type(ty)
                    .offline(false)
                    .build(),
            )],
        )
        .await;
    }

    let receipts = chat_store.receipts(&group, "OUT-G-AD").await.unwrap();
    let mut participants: Vec<String> = receipts.iter().map(|r| r.user_jid.to_string()).collect();
    participants.dedup();
    assert_eq!(
        participants,
        vec!["111000011112222@lid"],
        "two devices are one member: {receipts:?}"
    );
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.status, r.timestamp.timestamp()))
            .collect::<Vec<_>>(),
        vec![
            (MessageStatus::Delivered, 1_700_000_200),
            (MessageStatus::Read, 1_700_000_300),
        ],
        "each state keeps the instant it happened: {receipts:?}"
    );
}

#[derive(diesel::QueryableByName, Debug)]
struct JidRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    jid: String,
}

#[derive(diesel::QueryableByName, Debug)]
struct ReceiptKeyRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    user_jid: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    receipt_type: i32,
}

/// The heal migration, replayed over rows the pre-fix writers left behind.
/// Migrations run at open, so the artifacts are seeded afterwards and the
/// statements re-applied — they are idempotent by construction.
#[tokio::test]
async fn migration_folds_device_suffixed_rows() {
    let (store, _chat_store) = test_store().await;

    store
        .shared()
        .run(|conn| {
            let seed = [
                // Phantom chat from a read-self, plus the real thread.
                "INSERT INTO chats (device_id, jid) VALUES (1, '10203040506070:48@lid')",
                "INSERT INTO chats (device_id, jid) VALUES (1, '10203040506070@lid')",
                // A device-keyed chat that somehow owns messages is left alone.
                "INSERT INTO chats (device_id, jid) VALUES (1, '20304050607080:7@lid')",
                "INSERT INTO messages (device_id, chat_jid, msg_id, sender_jid, timestamp_ms, kind) \
                 VALUES (1, '20304050607080:7@lid', 'M-1', '', 1, 'text')",
                // Contact reachable only under the device key…
                "INSERT INTO contacts (device_id, jid, push_name) VALUES (1, '30405060708090:5@lid', 'Bob')",
                // …and one whose bare row already exists and must win.
                "INSERT INTO contacts (device_id, jid, push_name) VALUES (1, '10203040506070:48@lid', 'stale')",
                "INSERT INTO contacts (device_id, jid, push_name) VALUES (1, '10203040506070@lid', 'Alice')",
                // Same participant split across phone and Web: Read wins.
                "INSERT INTO message_receipts (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms) \
                 VALUES (1, '120363000000000001@g.us', 'G-1', '111000011112222@lid', 3, 10)",
                "INSERT INTO message_receipts (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms) \
                 VALUES (1, '120363000000000001@g.us', 'G-1', '111000011112222:48@lid', 4, 20)",
                // Two device rows and no bare row: the highest still survives.
                "INSERT INTO message_receipts (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms) \
                 VALUES (1, '120363000000000001@g.us', 'G-1', '222000011112222:3@lid', 4, 30)",
                "INSERT INTO message_receipts (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms) \
                 VALUES (1, '120363000000000001@g.us', 'G-1', '222000011112222:9@lid', 3, 40)",
            ];
            for stmt in seed {
                diesel::sql_query(stmt)
                    .execute(conn)
                    .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            }
            // Run the file the way the migration harness does, statements and
            // comments included.
            diesel::connection::SimpleConnection::batch_execute(
                conn,
                include_str!("../migrations/2026-07-24-000000_bare_identity_keys/up.sql"),
            )
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .unwrap();

    let (chats, contacts, receipts) = store
        .shared()
        .run(|conn| {
            let chats: Vec<JidRow> =
                diesel::sql_query("SELECT jid FROM chats WHERE device_id = 1 ORDER BY jid")
                    .load(conn)
                    .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            let contacts: Vec<JidRow> = diesel::sql_query(
                "SELECT jid || '=' || push_name AS jid FROM contacts WHERE device_id = 1 ORDER BY jid",
            )
            .load(conn)
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            let receipts: Vec<ReceiptKeyRow> = diesel::sql_query(
                "SELECT user_jid, receipt_type FROM message_receipts WHERE device_id = 1 ORDER BY user_jid",
            )
            .load(conn)
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))?;
            Ok((chats, contacts, receipts))
        })
        .await
        .unwrap();

    assert_eq!(
        chats.iter().map(|r| r.jid.as_str()).collect::<Vec<_>>(),
        ["10203040506070@lid", "20304050607080:7@lid"]
    );
    assert_eq!(
        contacts.iter().map(|r| r.jid.as_str()).collect::<Vec<_>>(),
        ["10203040506070@lid=Alice", "30405060708090@lid=Bob"]
    );
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.user_jid.as_str(), r.receipt_type))
            .collect::<Vec<_>>(),
        [("111000011112222@lid", 4), ("222000011112222@lid", 4)]
    );
}

// ---------------------------------------------------------------------------
// Arrival ordering (#1134)
// ---------------------------------------------------------------------------

/// The server's `t` is whole seconds, so a live back-and-forth lands several
/// messages on the same `timestamp_ms`. The tiebreak used to be `msg_id`, which
/// encodes nothing about time and is biased: this library stamps a constant
/// `3EB0` prefix on the ids it generates, while a peer's ids are effectively
/// uniform hex, so descending id order put the peer's message on top for ~75%
/// of ties — a reply rendering above the message it answers, every time.
#[tokio::test]
async fn same_second_messages_order_by_arrival_not_id() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let second = 1_785_101_675;

    // Inbound first. Its id sorts ABOVE an outgoing `3EB0…` id, which is
    // exactly the case that used to inverted the pair.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("1st"),
            incoming_info(PEER, PEER, "AAF59A7CB022679C9C44060A10C25026", second),
        )],
    )
    .await;
    chat_store
        .record_outgoing(
            &chat,
            "3EB025E0465016A858A333",
            &wa::Message::text("2nd"),
            Utc.timestamp_opt(second, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let messages = chat_store.messages(&chat, None, 10).await.unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|m| m.text.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["2nd", "1st"],
        "the reply is the newer message and must render below nothing"
    );
    assert!(
        messages[0].seq > messages[1].seq,
        "the arrival counter is what breaks the tie"
    );

    // The same tie decides the chat-list preview.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_preview.as_deref(), Some("2nd"));
}

/// A page boundary landing inside a same-second run must neither skip nor
/// repeat: the keyset filter has to mirror the sort's tiebreak exactly.
#[tokio::test]
async fn pagination_is_exact_across_a_same_second_run() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    // Six messages sharing one second, ids deliberately out of arrival order.
    let ids = ["F1", "A2", "3EB003", "B4", "3EB005", "C6"];
    let events: Vec<Event> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            message_event(
                wa::Message::text(format!("m{i}")),
                incoming_info(PEER, PEER, id, 1_700_000_000),
            )
        })
        .collect();
    feed(&chat_store, events).await;

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = chat_store.messages(&chat, cursor.take(), 2).await.unwrap();
        if page.is_empty() {
            break;
        }
        cursor = page.last().map(Into::into);
        seen.extend(page.into_iter().map(|m| m.text.unwrap()));
    }
    assert_eq!(seen, ["m5", "m4", "m3", "m2", "m1", "m0"]);
}

// ---------------------------------------------------------------------------
// Chat list: point lookup and keyset paging (#1141)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chat_point_lookup_resolves_either_identity() {
    let (store, chat_store) = test_store().await;
    add_lid_mapping(&store).await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("olá"),
            incoming_info(PEER, PEER, "MSG-PT-1", 1_700_000_000),
        )],
    )
    .await;

    let by_pn = chat_store.chat(&jid(PEER)).await.unwrap().expect("by pn");
    assert_eq!(by_pn.last_message_preview.as_deref(), Some("olá"));
    assert_eq!(by_pn.unread_count, 1);

    // The peer's other identity addresses the same thread.
    let by_lid = chat_store
        .chat(&jid(PEER_LID))
        .await
        .unwrap()
        .expect("by lid");
    assert_eq!(by_lid.jid, by_pn.jid);

    assert!(
        chat_store
            .chat(&jid("559900009999@s.whatsapp.net"))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn chat_list_pages_across_the_pinned_boundary() {
    let (_store, chat_store) = test_store().await;

    // Four chats, newest last, with the two oldest pinned so they lead.
    let peers: Vec<String> = (1..=4)
        .map(|i| format!("55990000000{i}@s.whatsapp.net"))
        .collect();
    let mut events = Vec::new();
    for (i, peer) in peers.iter().enumerate() {
        events.push(message_event(
            wa::Message::text(format!("m{i}")),
            incoming_info(peer, peer, &format!("MSG-PG-{i}"), 1_700_000_000 + i as i64),
        ));
    }
    // Pin peer 0 then peer 1, so peer 1 (pinned later) sorts first.
    for (i, peer) in peers.iter().take(2).enumerate() {
        events.push(Event::PinUpdate(
            wacore::types::events::PinUpdate::builder()
                .jid(jid(peer))
                .timestamp(Utc.timestamp_opt(1_700_000_500 + i as i64, 0).unwrap())
                .action(Box::new(wa::sync_action_value::PinAction {
                    pinned: Some(true),
                }))
                .from_full_sync(false)
                .build(),
        ));
    }
    feed(&chat_store, events).await;

    let whole = chat_store.chats(false, 10).await.unwrap();
    let expected: Vec<String> = whole.iter().map(|c| c.jid.to_string()).collect();
    assert_eq!(
        expected,
        vec![
            peers[1].clone(), // pinned last -> first
            peers[0].clone(),
            peers[3].clone(), // then by activity, newest first
            peers[2].clone(),
        ]
    );

    // Paging one at a time reproduces that order exactly, crossing from the
    // pinned run into the activity run without skipping or repeating.
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = None;
    loop {
        let page = chat_store
            .chats_page(false, cursor.take(), 1)
            .await
            .unwrap();
        if page.is_empty() {
            break;
        }
        cursor = page.last().map(Into::into);
        seen.extend(page.iter().map(|c| c.jid.to_string()));
    }
    assert_eq!(seen, expected);
}

// ---------------------------------------------------------------------------
// Server ack racing its own outgoing row (#1142)
// ---------------------------------------------------------------------------

/// `Event::ServerAck` is dispatched on the socket-read path while
/// `send_message` returns at the stanza write, so a host that records its
/// outgoing message after the send resolves can see the ack first. That ack
/// used to be dropped silently, leaving the row on a `pending` clock forever
/// and never applying the server's authoritative timestamp.
#[tokio::test]
async fn server_ack_arriving_before_its_outgoing_row_is_applied_on_insert() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let server_timestamp = Utc.timestamp_opt(1_700_000_222, 0).unwrap();

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-RACE".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .timestamp(server_timestamp)
                .build(),
        )],
    )
    .await;
    // Nothing to apply it to yet.
    assert!(
        chat_store
            .message(&chat, "OUT-RACE")
            .await
            .unwrap()
            .is_none()
    );

    chat_store
        .record_outgoing(
            &chat,
            "OUT-RACE",
            &wa::Message::text("beat me to it"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let msg = chat_store
        .message(&chat, "OUT-RACE")
        .await
        .unwrap()
        .expect("row");
    assert_eq!(msg.status, MessageStatus::ServerAck);
    assert_eq!(
        msg.timestamp, server_timestamp,
        "the held ack also carries the server's send clock"
    );
}

/// A nack that beats its row must land as ERROR, not as a silent pending row.
#[tokio::test]
async fn deferred_nack_marks_the_row_as_error() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-NACK-RACE".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .error("473".to_string())
                .build(),
        )],
    )
    .await;
    chat_store
        .record_outgoing(
            &chat,
            "OUT-NACK-RACE",
            &wa::Message::text("refused"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let msg = chat_store
        .message(&chat, "OUT-NACK-RACE")
        .await
        .unwrap()
        .expect("row");
    assert_eq!(msg.status, MessageStatus::Error);
}

/// A held ack belongs to one id only; an unrelated send must not consume it.
#[tokio::test]
async fn deferred_ack_only_matches_its_own_message() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-WAITING".to_string())
                .class("message".to_string())
                .from(chat.clone())
                .build(),
        )],
    )
    .await;
    for id in ["OUT-OTHER", "OUT-WAITING"] {
        chat_store
            .record_outgoing(
                &chat,
                id,
                &wa::Message::text(id),
                Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
            )
            .unwrap();
    }
    chat_store.flush().await.unwrap();

    assert_eq!(
        chat_store
            .message(&chat, "OUT-OTHER")
            .await
            .unwrap()
            .unwrap()
            .status,
        MessageStatus::Pending
    );
    assert_eq!(
        chat_store
            .message(&chat, "OUT-WAITING")
            .await
            .unwrap()
            .unwrap()
            .status,
        MessageStatus::ServerAck
    );
}

/// An ack whose id matches several outgoing rows cannot be attributed to any
/// of them, and waiting cannot disambiguate it. It must be dropped outright —
/// deferring it would arm it for whichever row next claims that id, turning a
/// deliberate refusal into a delayed mis-apply.
#[tokio::test]
async fn ambiguous_ack_is_dropped_rather_than_deferred() {
    let (_store, chat_store) = test_store().await;
    let other = jid("559900000002@s.whatsapp.net");
    let third = jid("559900000003@s.whatsapp.net");
    let sent_at = Utc.timestamp_opt(1_700_000_100, 0).unwrap();

    // The same id under two chats: no ack can name one of them.
    for chat in [&jid(PEER), &other] {
        chat_store
            .record_outgoing(chat, "OUT-DUP", &wa::Message::text("dup"), sent_at)
            .unwrap();
    }
    chat_store.flush().await.unwrap();

    // An ack with no chat identity, so resolution falls to the id alone.
    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-DUP".to_string())
                .class("message".to_string())
                .build(),
        )],
    )
    .await;
    for chat in [&jid(PEER), &other] {
        assert_eq!(
            chat_store
                .message(chat, "OUT-DUP")
                .await
                .unwrap()
                .unwrap()
                .status,
            MessageStatus::Pending,
            "an unattributable ack lifts nothing"
        );
    }

    // And it is not waiting in the wings: a later row reusing the id stays
    // pending too.
    chat_store
        .record_outgoing(&third, "OUT-DUP", &wa::Message::text("dup"), sent_at)
        .unwrap();
    chat_store.flush().await.unwrap();
    assert_eq!(
        chat_store
            .message(&third, "OUT-DUP")
            .await
            .unwrap()
            .unwrap()
            .status,
        MessageStatus::Pending
    );
}

/// An ack that names its chat must stay inside it. Ids are sender-chosen and
/// unique only within a chat, so a same-id row in an unrelated thread is a
/// different message — resolving to it would acknowledge the wrong send and
/// leave the real one pending.
#[tokio::test]
async fn a_named_ack_does_not_resolve_to_another_chats_row() {
    let (_store, chat_store) = test_store().await;
    let named = jid(PEER);
    let other = jid("559900000002@s.whatsapp.net");
    let sent_at = Utc.timestamp_opt(1_700_000_100, 0).unwrap();

    // Only the OTHER chat has a row under this id.
    chat_store
        .record_outgoing(&other, "OUT-CROSS", &wa::Message::text("theirs"), sent_at)
        .unwrap();
    chat_store.flush().await.unwrap();

    feed(
        &chat_store,
        [Event::ServerAck(
            ServerAck::builder()
                .id("OUT-CROSS".to_string())
                .class("message".to_string())
                .from(named.clone())
                .timestamp(Utc.timestamp_opt(1_700_000_222, 0).unwrap())
                .build(),
        )],
    )
    .await;
    let untouched = chat_store
        .message(&other, "OUT-CROSS")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        untouched.status,
        MessageStatus::Pending,
        "an ack for another chat must not lift this row"
    );
    assert_eq!(
        untouched.timestamp, sent_at,
        "nor rewrite its clock to that ack's server time"
    );

    // It was held for the chat it named, so that chat's send still gets it.
    chat_store
        .record_outgoing(&named, "OUT-CROSS", &wa::Message::text("mine"), sent_at)
        .unwrap();
    chat_store.flush().await.unwrap();
    assert_eq!(
        chat_store
            .message(&named, "OUT-CROSS")
            .await
            .unwrap()
            .unwrap()
            .status,
        MessageStatus::ServerAck
    );
}

// ---------------------------------------------------------------------------
// Hook-committed batches (#1140)
// ---------------------------------------------------------------------------

fn hook_committed_event(id: &str) -> Event {
    Event::Messages(
        MessageBatch::builder()
            .messages(Arc::from([InboundMessage::builder()
                .message(Arc::new(wa::Message::text("already durable")))
                .info(Arc::new(incoming_info(PEER, PEER, id, 1_700_000_000)))
                .build()]))
            .origin(BatchOrigin::Live)
            .hook_committed(true)
            .build(),
    )
}

/// Once the host declares that its hook feeds this store, a batch the hook
/// committed must not be applied a second time — the duplicate pass is a full
/// proto UPDATE plus an FTS delete+insert plus a doubled invalidation fan-out,
/// not a cheap no-op.
#[tokio::test]
async fn hook_committed_batch_is_skipped_when_opted_in() {
    let (_store, chat_store) = test_store().await;
    chat_store.skip_hook_committed_batches(true);

    feed(&chat_store, [hook_committed_event("MSG-HOOKED")]).await;

    assert!(chat_store.chats(false, 10).await.unwrap().is_empty());
    assert!(
        chat_store
            .message(&jid(PEER), "MSG-HOOKED")
            .await
            .unwrap()
            .is_none()
    );
}

/// The marker alone means "some hook committed this", not "this store already
/// has it". A host whose hook persists elsewhere still needs every batch, so
/// the default must materialize it — skipping would lose acknowledged
/// messages out of history, previews and subscriptions.
#[tokio::test]
async fn hook_committed_batch_is_materialized_by_default() {
    let (_store, chat_store) = test_store().await;

    feed(&chat_store, [hook_committed_event("MSG-OTHER-HOOK")]).await;

    assert_eq!(
        chat_store
            .message(&jid(PEER), "MSG-OTHER-HOOK")
            .await
            .unwrap()
            .expect("a hook that writes elsewhere leaves this store the only materializer")
            .text
            .as_deref(),
        Some("already durable")
    );
}

/// The producers that bypass the commit pipeline (newsletters, PDO recovery)
/// leave the marker unset, and this handler stays their only materializer.
#[tokio::test]
async fn unmarked_batch_is_still_materialized() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("only copy"),
            incoming_info(PEER, PEER, "MSG-UNHOOKED", 1_700_000_000),
        )],
    )
    .await;

    assert_eq!(
        chat_store
            .message(&jid(PEER), "MSG-UNHOOKED")
            .await
            .unwrap()
            .expect("row")
            .text
            .as_deref(),
        Some("only copy")
    );
}

// ---------------------------------------------------------------------------
// Scoped and bounded full-text search (#1147)
// ---------------------------------------------------------------------------

#[cfg(feature = "search")]
#[tokio::test]
async fn search_can_be_scoped_to_one_chat() {
    let (_store, chat_store) = test_store().await;
    let other = "559900000002@s.whatsapp.net";

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("orçamento aprovado"),
                incoming_info(PEER, PEER, "MSG-SC1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("orçamento recusado"),
                incoming_info(other, other, "MSG-SC2", 1_700_000_001),
            ),
        ],
    )
    .await;

    // Unscoped sees both threads.
    assert_eq!(
        chat_store
            .search_messages("orçamento", 10)
            .await
            .unwrap()
            .len(),
        2
    );

    // Scoped sees only its own, without the caller over-fetching and filtering.
    let scoped = chat_store
        .search_messages_in_chat(&jid(PEER), "orçamento", 10)
        .await
        .unwrap();
    assert_eq!(
        scoped.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MSG-SC1"]
    );
}

/// A chat addressed by either of the peer's identities is the same thread, so
/// the scope has to resolve through the alias like every other read.
#[cfg(feature = "search")]
#[tokio::test]
async fn scoped_search_resolves_the_peer_alias() {
    let (store, chat_store) = test_store().await;
    add_lid_mapping(&store).await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("combinado então"),
            incoming_info(PEER, PEER, "MSG-SC-LID", 1_700_000_000),
        )],
    )
    .await;

    let by_lid = chat_store
        .search_messages_in_chat(&jid(PEER_LID), "combinado", 10)
        .await
        .unwrap();
    assert_eq!(
        by_lid.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MSG-SC-LID"]
    );
}

/// Hits come back fully hydrated from one statement rather than a point query
/// each, so everything a caller reads off a hit still has to be there.
#[cfg(feature = "search")]
#[tokio::test]
async fn search_hits_are_fully_hydrated() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("documento anexado"),
            incoming_info(PEER, PEER, "MSG-HY", 1_700_000_000),
        )],
    )
    .await;

    let hits = chat_store.search_messages("documento", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert_eq!(hit.id, "MSG-HY");
    assert_eq!(hit.chat_jid, jid(PEER));
    assert_eq!(hit.sender_jid, jid(PEER));
    assert_eq!(hit.text.as_deref(), Some("documento anexado"));
    assert_eq!(hit.kind, MessageKind::Text);
    assert!(!hit.from_me);
    assert!(hit.seq > 0, "arrival sequence survives the bulk load");
    // The proto still decodes — hydration did not drop the blob.
    assert_eq!(
        hit.message
            .as_ref()
            .expect("decoded proto")
            .conversation
            .as_deref(),
        Some("documento anexado")
    );
}

/// A one- or two-character prefix skips relevance ranking, which would
/// otherwise have to score every row it matches before `LIMIT` discarded any.
/// It still has to return that many hits, newest first.
#[cfg(feature = "search")]
#[tokio::test]
async fn a_short_prefix_returns_newest_first_within_its_limit() {
    let (_store, chat_store) = test_store().await;

    let events: Vec<Event> = (0..6)
        .map(|i| {
            message_event(
                wa::Message::text(format!("hoje{i} agora")),
                incoming_info(PEER, PEER, &format!("MSG-SP-{i}"), 1_700_000_000 + i),
            )
        })
        .collect();
    feed(&chat_store, events).await;

    let hits = chat_store.search_messages("h", 3).await.unwrap();
    assert_eq!(
        hits.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MSG-SP-5", "MSG-SP-4", "MSG-SP-3"],
        "newest first, capped at the limit"
    );

    // One short token demotes the WHOLE query to recency — the rule is `all`,
    // not `any`, so a mixed-length search must not quietly go back to ranking.
    let mixed = chat_store.search_messages("hoje a", 3).await.unwrap();
    assert_eq!(
        mixed.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MSG-SP-5", "MSG-SP-4", "MSG-SP-3"]
    );

    // A long-enough term still ranks, and still finds its message.
    let ranked = chat_store.search_messages("hoje4", 10).await.unwrap();
    assert_eq!(
        ranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MSG-SP-4"]
    );
}

#[cfg(feature = "search")]
#[tokio::test]
async fn scoped_search_rejects_an_empty_query_like_the_unscoped_one() {
    let (_store, chat_store) = test_store().await;
    assert!(
        chat_store
            .search_messages_in_chat(&jid(PEER), "   ", 10)
            .await
            .is_err()
    );
    assert_eq!(
        chat_store
            .search_messages_in_chat(&jid(PEER), "olá", 0)
            .await
            .unwrap()
            .len(),
        0
    );
}

// ---------------------------------------------------------------------------
// Unavailable fanouts keep their type (#1150)
// ---------------------------------------------------------------------------

fn unavailable_event(
    id: &str,
    ts: i64,
    unavailable_type: wacore::types::events::UnavailableType,
) -> Event {
    Event::UndecryptableMessage(
        wacore::types::events::UndecryptableMessage::builder()
            .info(Arc::new(incoming_info(PEER, PEER, id, ts)))
            .is_unavailable(true)
            .unavailable_type(unavailable_type)
            .decrypt_fail_mode(wacore::types::events::DecryptFailMode::Show)
            .build(),
    )
}

/// The three unrecoverable fanouts are content the phone never shares with a
/// companion, so their rows are permanent by design. Flattening them into the
/// generic placeholder left a frontend rendering "waiting for this message"
/// for something that will never arrive.
#[tokio::test]
async fn unrecoverable_fanouts_keep_their_type() {
    use wacore::types::events::UnavailableType;

    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    let cases = [
        (UnavailableType::ViewOnce, MessageKind::ViewOnce, "MSG-VO"),
        (UnavailableType::Hosted, MessageKind::Hosted, "MSG-HO"),
        (UnavailableType::Bot, MessageKind::Bot, "MSG-BO"),
    ];

    for (at, (unavailable_type, _, id)) in cases.iter().enumerate() {
        feed(
            &chat_store,
            [unavailable_event(
                id,
                1_700_000_000 + at as i64,
                *unavailable_type,
            )],
        )
        .await;
    }

    for (unavailable_type, expected, id) in cases {
        let row = chat_store
            .message(&chat, id)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{unavailable_type:?} should materialize"));
        assert_eq!(row.kind, expected, "{unavailable_type:?}");
        assert!(row.message.is_none(), "{unavailable_type:?}: no content");
    }

    // The chat preview carries it too, so a chat list can render the chip
    // without opening the thread.
    let chats = chat_store.chats(false, 10).await.unwrap();
    assert_eq!(chats[0].last_message_kind, Some(MessageKind::Bot));
}

/// A plain fanout is still recoverable — PDO may fill it in — so it must keep
/// the placeholder kind and the "yet" it implies.
#[tokio::test]
async fn a_recoverable_fanout_stays_undecryptable() {
    use wacore::types::events::UnavailableType;

    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);
    feed(
        &chat_store,
        [unavailable_event(
            "MSG-PLAIN",
            1_700_000_000,
            UnavailableType::Unknown,
        )],
    )
    .await;

    assert_eq!(
        chat_store
            .message(&chat, "MSG-PLAIN")
            .await
            .unwrap()
            .unwrap()
            .kind,
        MessageKind::Undecryptable
    );
}

/// The labels are on-disk values, so a reader on an older build still round
/// trips them rather than losing the row.
#[test]
fn unavailable_kind_labels_are_stable() {
    for (kind, label) in [
        (MessageKind::ViewOnce, "view_once"),
        (MessageKind::Hosted, "hosted"),
        (MessageKind::Bot, "bot"),
        (MessageKind::Undecryptable, "undecryptable"),
    ] {
        assert_eq!(kind.as_str(), label);
    }
}

// --- Session-wide arrival feed ---------------------------------------------

/// One page spans every chat, in the order the rows landed — the read an
/// external reconciler needs and could otherwise only fake by paging each
/// thread.
#[tokio::test]
async fn arrival_feed_interleaves_every_chat_newest_first() {
    let (_store, chat_store) = test_store().await;
    let other = "559900000002@s.whatsapp.net";

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("peer one"),
                incoming_info(PEER, PEER, "A-1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("group"),
                incoming_info(GROUP, PEER, "G-1", 1_700_000_001),
            ),
            message_event(
                wa::Message::text("peer two"),
                incoming_info(other, other, "B-1", 1_700_000_002),
            ),
        ],
    )
    .await;

    let page = chat_store.messages_by_arrival(None, 10).await.unwrap();
    assert_eq!(
        page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["B-1", "G-1", "A-1"]
    );
    // Every chat is represented, and `seq` really is descending.
    assert_eq!(page[0].chat_jid, jid(other));
    assert_eq!(page[1].chat_jid, jid(GROUP));
    assert!(page[0].seq > page[1].seq && page[1].seq > page[2].seq);
}

/// Pages tile the feed: walking the cursor to exhaustion yields every row
/// exactly once, which is the whole contract a resumable consumer relies on.
#[tokio::test]
async fn arrival_feed_pages_without_gaps_or_repeats() {
    let (_store, chat_store) = test_store().await;

    let events: Vec<_> = (0..7)
        .map(|i| {
            let chat = if i % 2 == 0 { PEER } else { GROUP };
            message_event(
                wa::Message::text("m"),
                incoming_info(chat, PEER, &format!("M-{i}"), 1_700_000_000 + i),
            )
        })
        .collect();
    feed(&chat_store, events).await;

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = chat_store.messages_by_arrival(cursor, 3).await.unwrap();
        let Some(last) = page.last() else { break };
        cursor = Some(last.into());
        seen.extend(page.iter().map(|m| m.id.clone()));
    }

    assert_eq!(
        seen,
        (0..7).rev().map(|i| format!("M-{i}")).collect::<Vec<_>>()
    );
}

/// The property that rules out a `timestamp_ms` cursor: history sync backfills
/// old conversations at NEW arrival positions. A timestamp-keyed poller would
/// file those rows behind its watermark and never look at them again; the
/// arrival feed puts them at the head, where the next pull sees them.
#[tokio::test]
async fn arrival_feed_surfaces_backfill_dated_before_the_last_page() {
    let (_store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("live"),
            incoming_info(PEER, PEER, "LIVE-1", 1_700_000_000),
        )],
    )
    .await;
    let watermark: whatsapp_rust_chat_store::ArrivalCursor =
        (&chat_store.messages_by_arrival(None, 10).await.unwrap()[0]).into();

    // Two years older than the live message, and it arrives now.
    let old_ts = 1_640_000_000u64;
    let history = wa::HistorySync {
        sync_type: wa::history_sync::HistorySyncType::RECENT,
        conversations: vec![wa::Conversation {
            id: GROUP.to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: MessageField::some(wa::WebMessageInfo {
                    key: MessageField::some(wa::MessageKey {
                        remote_jid: Some(GROUP.into()),
                        from_me: Some(false),
                        id: Some("BACKFILL-1".into()),
                        participant: Some(PEER.into()),
                    }),
                    message: MessageField::from_box(Box::new(wa::Message::text("old"))),
                    message_timestamp: Some(old_ts),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    feed(&chat_store, [history_sync_event(history)]).await;

    let head = chat_store.messages_by_arrival(None, 10).await.unwrap();
    assert_eq!(head[0].id, "BACKFILL-1", "backfill sits at the newest end");
    assert!(
        head[0].timestamp < head[1].timestamp,
        "and it is genuinely older than the row it outranks"
    );
    assert!(
        head[0].seq > watermark.seq,
        "so a stale cursor still sees it"
    );
}

/// The wall-clock window is half-open, `since <= timestamp < until`, so a
/// consumer walking adjacent windows neither double-counts nor drops a row.
#[tokio::test]
async fn arrival_feed_window_is_half_open() {
    let (_store, chat_store) = test_store().await;

    let events: Vec<_> = (0..4)
        .map(|i| {
            message_event(
                wa::Message::text("m"),
                incoming_info(PEER, PEER, &format!("W-{i}"), 1_700_000_000 + i),
            )
        })
        .collect();
    feed(&chat_store, events).await;

    let at = |secs: i64| Utc.timestamp_opt(secs, 0).unwrap();
    let ids = |page: Vec<whatsapp_rust_chat_store::StoredMessage>| {
        page.into_iter().map(|m| m.id).collect::<Vec<_>>()
    };

    let window = chat_store
        .messages_by_arrival_in_range(None, Some(at(1_700_000_001)), Some(at(1_700_000_003)), 10)
        .await
        .unwrap();
    assert_eq!(ids(window), ["W-2", "W-1"]);

    // Bounds are independent.
    let open_end = chat_store
        .messages_by_arrival_in_range(None, Some(at(1_700_000_002)), None, 10)
        .await
        .unwrap();
    assert_eq!(ids(open_end), ["W-3", "W-2"]);
    let open_start = chat_store
        .messages_by_arrival_in_range(None, None, Some(at(1_700_000_001)), 10)
        .await
        .unwrap();
    assert_eq!(ids(open_start), ["W-0"]);

    // The cursor still bounds the window rather than being overridden by it.
    let resumed = chat_store
        .messages_by_arrival_in_range(
            Some((&chat_store.messages_by_arrival(None, 10).await.unwrap()[0]).into()),
            Some(at(1_700_000_001)),
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(ids(resumed), ["W-2", "W-1"]);
}

/// Tombstones are rows too, and a revoke does NOT reorder them. Both halves
/// matter and they pull in opposite directions: the feed carries the tombstone,
/// so a full pass sees the withdrawal, but the revoke rewrites the row in place
/// and leaves `seq` where the insert put it — so a consumer tailing only the
/// head walks straight past a message that was revoked after it read it. That
/// is the boundary between this feed and `subscribe()`, and it is documented on
/// `messages_by_arrival_in_range` because it is not guessable from the name.
#[tokio::test]
async fn arrival_feed_carries_tombstones_without_reordering_them() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "REVOKED-1",
            &wa::Message::text("oops"),
            Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();
    let before = chat_store.messages_by_arrival(None, 10).await.unwrap();
    let seq_before = before[0].seq;

    // A later message, so "did the revoke move it to the head?" has an answer.
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("after"),
            incoming_info(PEER, PEER, "LATER-1", 1_700_000_005),
        )],
    )
    .await;
    chat_store
        .record_revoke(
            &chat,
            "REVOKED-1",
            Utc.timestamp_opt(1_700_000_010, 0).unwrap(),
        )
        .unwrap();
    chat_store.flush().await.unwrap();

    let page = chat_store.messages_by_arrival(None, 10).await.unwrap();
    assert_eq!(
        page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["LATER-1", "REVOKED-1"],
        "the revoke must not move the row it tombstoned"
    );
    let tombstone = &page[1];
    assert!(tombstone.revoked);
    assert!(tombstone.message.is_none());
    assert_eq!(tombstone.seq, seq_before, "arrival position is immutable");
}

/// Another device's history in the same file is not this session's feed. The
/// `device_id` predicate is the only thing scoping a read that has no chat key
/// to narrow it, so it gets its own test.
#[tokio::test]
async fn arrival_feed_is_scoped_to_the_session_device() {
    let (store, chat_store) = test_store().await;

    feed(
        &chat_store,
        [message_event(
            wa::Message::text("mine"),
            incoming_info(PEER, PEER, "MINE-1", 1_700_000_000),
        )],
    )
    .await;

    let sibling = store.device_id() + 1;
    store
        .shared()
        .run(move |conn| {
            diesel::sql_query(format!(
                "INSERT INTO messages (device_id, chat_jid, msg_id, sender_jid, from_me, \
                 timestamp_ms, kind, text_content, status, starred, revoked) \
                 VALUES ({sibling}, '{PEER}', 'THEIRS-1', '{PEER}', 0, 1700000001000, \
                 'text', 'theirs', 2, 0, 0)"
            ))
            .execute(conn)
            .map(|_| ())
            .map_err(|e| wacore::store::error::StoreError::Database(Box::new(e)))
        })
        .await
        .unwrap();

    let page = chat_store.messages_by_arrival(None, 10).await.unwrap();
    assert_eq!(
        page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["MINE-1"],
        "the sibling device's row has the newer seq and must still be absent"
    );
}

/// A limit of zero (or a negative one, which SQLite reads as unbounded) returns
/// nothing rather than the whole table.
#[tokio::test]
async fn arrival_feed_rejects_an_unbounded_limit() {
    let (_store, chat_store) = test_store().await;
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("m"),
            incoming_info(PEER, PEER, "L-1", 1_700_000_000),
        )],
    )
    .await;

    assert!(
        chat_store
            .messages_by_arrival(None, 0)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        chat_store
            .messages_by_arrival(None, -1)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Sub-millisecond bounds are honored exactly. `Utc::now()` carries
/// nanoseconds, so a caller asking for "the last hour" hits this on every call;
/// truncating the bound gets both ends of the half-open window backwards.
#[tokio::test]
async fn arrival_feed_window_honors_sub_millisecond_bounds() {
    let (_store, chat_store) = test_store().await;
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("m"),
            incoming_info(PEER, PEER, "SUB-1", 1_700_000_000),
        )],
    )
    .await;
    // The stored row sits exactly on a whole second, so on a whole millisecond.
    let on_the_ms = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let just_after = on_the_ms + chrono::Duration::microseconds(500);

    // `since` half a microsecond past the row excludes it: the row precedes
    // the bound. Truncation would have kept it.
    assert!(
        chat_store
            .messages_by_arrival_in_range(None, Some(just_after), None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    // ...and `until` at the same instant includes it, for the same reason.
    assert_eq!(
        chat_store
            .messages_by_arrival_in_range(None, None, Some(just_after), 10)
            .await
            .unwrap()
            .len(),
        1
    );
    // A bound exactly on the row keeps the half-open contract: `since`
    // includes, `until` excludes.
    assert_eq!(
        chat_store
            .messages_by_arrival_in_range(None, Some(on_the_ms), None, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        chat_store
            .messages_by_arrival_in_range(None, None, Some(on_the_ms), 10)
            .await
            .unwrap()
            .is_empty()
    );
}

/// The fact that rules out a remembered `seq`: SQLite assigns the implicit
/// rowid as `max(rowid) + 1`, so deleting the newest message hands its number
/// to the next arrival. A consumer that stopped at a saved watermark would read
/// that brand-new message as already seen and drop it — silently, and after an
/// ordinary delete-for-me, not an exotic one.
#[tokio::test]
async fn a_new_message_can_land_at_a_previously_used_seq() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    feed(
        &chat_store,
        [
            message_event(
                wa::Message::text("first"),
                incoming_info(PEER, PEER, "REUSE-1", 1_700_000_000),
            ),
            message_event(
                wa::Message::text("newest"),
                incoming_info(PEER, PEER, "REUSE-2", 1_700_000_001),
            ),
        ],
    )
    .await;
    let watermark = chat_store.messages_by_arrival(None, 10).await.unwrap()[0].seq;

    feed(
        &chat_store,
        [Event::DeleteMessageForMeUpdate(
            wacore::types::events::DeleteMessageForMeUpdate::builder()
                .chat_jid(chat.clone())
                .message_id("REUSE-2".to_string())
                .from_me(false)
                .timestamp(Utc.timestamp_opt(1_700_000_002, 0).unwrap())
                .action(Box::new(
                    wa::sync_action_value::DeleteMessageForMeAction::default(),
                ))
                .from_full_sync(false)
                .build(),
        )],
    )
    .await;
    feed(
        &chat_store,
        [message_event(
            wa::Message::text("genuinely new"),
            incoming_info(PEER, PEER, "REUSE-3", 1_700_000_002),
        )],
    )
    .await;

    let page = chat_store.messages_by_arrival(None, 10).await.unwrap();
    assert_eq!(page[0].id, "REUSE-3");
    assert!(
        page[0].seq <= watermark,
        "a new arrival reused the deleted row's seq ({} vs watermark {}), which \
         is why the documented loop stops on content and not on a saved seq",
        page[0].seq,
        watermark
    );
}
