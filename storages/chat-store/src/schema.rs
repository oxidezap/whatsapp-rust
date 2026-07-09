diesel::table! {
    chats (device_id, jid) {
        device_id -> Integer,
        jid -> Text,
        name -> Nullable<Text>,
        last_message_ts -> BigInt,
        last_message_preview -> Nullable<Text>,
        last_message_kind -> Nullable<Text>,
        unread_count -> Integer,
        pinned_at -> Nullable<BigInt>,
        muted_until -> Nullable<BigInt>,
        archived -> Bool,
        ephemeral_expiration -> Nullable<Integer>,
        read_boundary_ms -> BigInt,
    }
}

diesel::table! {
    messages (device_id, chat_jid, msg_id) {
        device_id -> Integer,
        chat_jid -> Text,
        msg_id -> Text,
        sender_jid -> Text,
        from_me -> Bool,
        timestamp_ms -> BigInt,
        kind -> Text,
        text_content -> Nullable<Text>,
        proto -> Nullable<Binary>,
        status -> Integer,
        starred -> Bool,
        edited_at_ms -> Nullable<BigInt>,
        revoked -> Bool,
    }
}

diesel::table! {
    reactions (device_id, chat_jid, msg_id, sender_jid) {
        device_id -> Integer,
        chat_jid -> Text,
        msg_id -> Text,
        sender_jid -> Text,
        emoji -> Text,
        ts_ms -> BigInt,
    }
}

diesel::table! {
    contacts (device_id, jid) {
        device_id -> Integer,
        jid -> Text,
        push_name -> Nullable<Text>,
        full_name -> Nullable<Text>,
        first_name -> Nullable<Text>,
        business_name -> Nullable<Text>,
    }
}

diesel::table! {
    message_receipts (device_id, chat_jid, msg_id, user_jid) {
        device_id -> Integer,
        chat_jid -> Text,
        msg_id -> Text,
        user_jid -> Text,
        receipt_type -> Integer,
        ts_ms -> BigInt,
    }
}

diesel::table! {
    media_refs (device_id, file_sha256) {
        device_id -> Integer,
        file_sha256 -> Binary,
        file_path -> Text,
        mime_type -> Nullable<Text>,
        size_bytes -> Nullable<BigInt>,
        downloaded_at_ms -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    messages,
    reactions,
    contacts,
    message_receipts
);
