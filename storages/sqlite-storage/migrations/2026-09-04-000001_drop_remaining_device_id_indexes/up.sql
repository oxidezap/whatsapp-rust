-- The last five standalone device_id indexes, dropped for the reason the two
-- 2026-09-03 migrations gave for the seven before them: each of these tables is
-- keyed PRIMARY KEY (..., device_id) and every query filters on that key, so the
-- index is one more b-tree write per inserted or deleted row that nothing
-- selective reads -- it holds a single distinct value per account. The only
-- device_id-alone scan is account teardown, where a table scan is fine.
--
-- `idx_base_keys_device` goes with them rather than being kept: the retention
-- sweep added in 2026-09-04-000000 reads (device_id, created_at), whose leading
-- column covers everything the single-column index could have answered.
DROP INDEX IF EXISTS idx_signed_prekeys_device_id;
DROP INDEX IF EXISTS idx_app_state_keys_device_id;
DROP INDEX IF EXISTS idx_app_state_versions_device_id;
DROP INDEX IF EXISTS idx_app_state_mutation_macs_device_id;
DROP INDEX IF EXISTS idx_base_keys_device;
