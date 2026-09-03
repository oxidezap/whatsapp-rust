-- `base_keys` rows are written when a peer's retry #2 arrives and deleted only
-- when a retry #3 follows for the same message, which is the uncommon case: the
-- table had no deletion path at all for its common one, and `created_at` (added
-- 2025-12-24) was written and read by nothing. The keepalive sweep now prunes on
-- it, so index it in the shape that prune uses -- device_id first (equality),
-- created_at second (range) -- the same order the `msg_secrets` and
-- `pending_inbound_messages` retention indexes adopted.
CREATE INDEX idx_base_keys_created ON base_keys (device_id, created_at);
