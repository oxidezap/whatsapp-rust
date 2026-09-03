-- `device_registry` is keyed PRIMARY KEY (user_id, device_id) and every query
-- filters on both, so the standalone device_id index is a third b-tree write
-- per row that nothing selective reads (one distinct value per account).
-- `timestamp` is only ever selected as a column and `updated_at` is written
-- and never read, so their indexes are two more b-trees no query plan uses.
-- Measured on a file-backed store: the usync batch write of 256 records goes
-- 4.56 ms -> 3.80 ms without them, a single record 119 us -> 79 us.
DROP INDEX IF EXISTS idx_device_registry_timestamp;
DROP INDEX IF EXISTS idx_device_registry_device;
DROP INDEX IF EXISTS idx_device_registry_updated_at;
