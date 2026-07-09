-- Chat/message history tables. They live in the SAME database file as the
-- whatsapp-rust-sqlite-storage tables (shared pool via SqliteStore::shared());
-- these table names are reserved by the chat-store crate.

CREATE TABLE chats (
    device_id INTEGER NOT NULL,
    jid TEXT NOT NULL,
    name TEXT,
    last_message_ts BIGINT NOT NULL DEFAULT 0,
    last_message_preview TEXT,
    last_message_kind TEXT,
    -- -1 = manually marked unread (WA Web convention)
    unread_count INTEGER NOT NULL DEFAULT 0,
    pinned_at BIGINT,
    muted_until BIGINT,
    archived BOOLEAN NOT NULL DEFAULT FALSE,
    ephemeral_expiration INTEGER,
    PRIMARY KEY (device_id, jid)
);

CREATE INDEX idx_chats_order ON chats (device_id, archived, last_message_ts DESC);

CREATE TABLE messages (
    device_id INTEGER NOT NULL,
    chat_jid TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    sender_jid TEXT NOT NULL,
    from_me BOOLEAN NOT NULL DEFAULT FALSE,
    timestamp_ms BIGINT NOT NULL,
    kind TEXT NOT NULL,
    text_content TEXT,
    -- wa::Message encoded with buffa; NULL for tombstones (revoked) and
    -- undecryptable placeholders. Source of truth: columns are projections.
    proto BLOB,
    -- WebMessageInfo.Status values: 0 error, 1 pending, 2 server ack,
    -- 3 delivered, 4 read, 5 played.
    status INTEGER NOT NULL DEFAULT 1,
    starred BOOLEAN NOT NULL DEFAULT FALSE,
    edited_at_ms BIGINT,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (device_id, chat_jid, msg_id)
);

CREATE INDEX idx_messages_chat_time ON messages (device_id, chat_jid, timestamp_ms DESC, msg_id DESC);
-- Server acks carry only the message id, not the chat.
CREATE INDEX idx_messages_by_id ON messages (device_id, msg_id);

CREATE TABLE reactions (
    device_id INTEGER NOT NULL,
    chat_jid TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    sender_jid TEXT NOT NULL,
    emoji TEXT NOT NULL,
    ts_ms BIGINT NOT NULL,
    PRIMARY KEY (device_id, chat_jid, msg_id, sender_jid)
);

CREATE TABLE contacts (
    device_id INTEGER NOT NULL,
    jid TEXT NOT NULL,
    push_name TEXT,
    full_name TEXT,
    first_name TEXT,
    business_name TEXT,
    PRIMARY KEY (device_id, jid)
);

-- Per-user delivery/read receipts (group read-by lists).
CREATE TABLE message_receipts (
    device_id INTEGER NOT NULL,
    chat_jid TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    user_jid TEXT NOT NULL,
    -- Same scale as messages.status: 3 delivered, 4 read, 5 played.
    receipt_type INTEGER NOT NULL,
    ts_ms BIGINT NOT NULL,
    PRIMARY KEY (device_id, chat_jid, msg_id, user_jid)
);

-- Downloaded-media cache index: content hash -> local file, so media survives
-- restarts and identical files are stored once.
CREATE TABLE media_refs (
    device_id INTEGER NOT NULL,
    file_sha256 BLOB NOT NULL,
    file_path TEXT NOT NULL,
    mime_type TEXT,
    size_bytes BIGINT,
    downloaded_at_ms BIGINT NOT NULL,
    PRIMARY KEY (device_id, file_sha256)
);
