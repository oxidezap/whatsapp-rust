-- The four Signal tables are keyed PRIMARY KEY (address|id, device_id), and
-- every hot query filters on that key, so the standalone device_id index
-- added with multi-account support only ever cost a third b-tree write per
-- inserted or deleted row. Nothing selective reads it: it has one distinct
-- value per account, and the only device_id-alone scans are account
-- teardown, where a table scan is fine.
DROP INDEX IF EXISTS idx_identities_device_id;
DROP INDEX IF EXISTS idx_sessions_device_id;
DROP INDEX IF EXISTS idx_prekeys_device_id;
DROP INDEX IF EXISTS idx_sender_keys_device_id;
