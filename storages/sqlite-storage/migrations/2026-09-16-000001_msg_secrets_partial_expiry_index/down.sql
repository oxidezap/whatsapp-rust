-- Restore the non-partial expiry index. Rows dropped from a partial index are
-- not deleted, so no data is involved; the rebuild just indexes them again.
DROP INDEX IF EXISTS idx_msg_secrets_expires;
CREATE INDEX idx_msg_secrets_expires ON msg_secrets (device_id, expires_at);
