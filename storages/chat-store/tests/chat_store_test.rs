//! Integration tests: real SqliteStore (in-memory), real writer task, events
//! fed through the public handler exactly as the client would.

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
async fn outgoing_status_advances_monotonically() {
    let (_store, chat_store) = test_store().await;
    let chat = jid(PEER);

    chat_store
        .record_outgoing(
            &chat,
            "OUT-1",
            &wa::Message::text("oi"),
            Utc.timestamp_opt(1_700_000_100, 0).unwrap(),
        )
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

    use buffa::Message as _;
    let raw = history.encode_to_vec();
    let compressed = {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    };
    feed(
        &chat_store,
        [Event::HistorySync(Box::new(LazyHistorySync::new(
            compressed.into(),
            raw.len(),
            wa::history_sync::HistorySyncType::RECENT as i32,
            None,
            None,
        )))],
    )
    .await;

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
                .source(MessageSource {
                    chat: group.clone(),
                    sender: jid(alice),
                    is_group: true,
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
