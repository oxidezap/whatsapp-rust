-- Switch msg_secret retention from insert-time created_at to a per-row
-- absolute expires_at deadline (0 = never). created_at was overwritten on
-- every conflict-update and edit re-persist, so no age-based TTL was sound.

ALTER TABLE msg_secrets ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0;

-- Backfill the pre-existing pairing-burst seed so it ages out under the
-- default text horizon (30d) once pruning runs, instead of either living
-- forever or being mass-deleted on the first sweep. created_at is the only
-- timestamp legacy rows carry. Secrets for messages older than the horizon
-- cannot unlock any add-on anyway, so reclaiming them loses nothing.
UPDATE msg_secrets SET expires_at = created_at + 2592000 WHERE expires_at = 0;

DROP INDEX IF EXISTS idx_msg_secrets_created;
CREATE INDEX idx_msg_secrets_expires ON msg_secrets (expires_at, device_id);
