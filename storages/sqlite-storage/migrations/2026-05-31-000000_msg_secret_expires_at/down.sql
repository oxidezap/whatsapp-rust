DROP INDEX IF EXISTS idx_msg_secrets_expires;
CREATE INDEX idx_msg_secrets_created ON msg_secrets (created_at, device_id);
ALTER TABLE msg_secrets DROP COLUMN message_ts;
ALTER TABLE msg_secrets DROP COLUMN expires_at;
