DROP INDEX IF EXISTS idx_chats_pinned;
DROP INDEX IF EXISTS idx_chats_order;
CREATE INDEX idx_chats_order ON chats (device_id, archived, last_message_ts DESC);
