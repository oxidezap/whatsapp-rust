-- Make the expiry index partial over the rows that can actually expire.
--
-- The prune is `device_id = ? AND expires_at <> 0 AND expires_at <= ?`. A row
-- with `expires_at = 0` is "never expire": the `<> 0` term filters it out of
-- every pass, so it never needs an index entry. Excluding those rows keeps the
-- index proportional to the expirable set instead of the whole table, which
-- matters under `MsgSecretPolicy::Full`, where every row is never-expire, and
-- removes the churn of writing index entries the prune will always skip.
-- `device_id` still leads, so the range scan stays localized to one account in
-- a multi-account database, and the point lookup is unaffected: it uses the
-- composite primary-key autoindex, not this one.
DROP INDEX IF EXISTS idx_msg_secrets_expires;
CREATE INDEX idx_msg_secrets_expires
    ON msg_secrets (device_id, expires_at)
    WHERE expires_at <> 0;
