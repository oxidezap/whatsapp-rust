DROP INDEX IF EXISTS idx_messages_chat_time;
CREATE INDEX idx_messages_chat_time ON messages (device_id, chat_jid, timestamp_ms DESC, msg_id DESC);
