-- Lookup the original participant spellings without scanning a chat's backlog.
-- No row, key or payload is rewritten.
CREATE INDEX idx_pending_inbound_message ON pending_inbound_messages (device_id, chat, id);
