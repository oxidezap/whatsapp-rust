-- Dropping this role marker while credentials remain could turn a primary
-- namespace into Web state. Even malformed or empty bindings must be cleared
-- explicitly before rollback. The migration harness runs this guard and the
-- schema change in one transaction.
CREATE TEMP TABLE __whatsapp_mobile_rollback_guard (
    bindings_remaining INTEGER NOT NULL
        CONSTRAINT mobile_session_bindings_must_be_cleared CHECK (bindings_remaining = 0)
);
INSERT INTO __whatsapp_mobile_rollback_guard (bindings_remaining)
    SELECT COUNT(*) FROM device WHERE mobile_session_binding IS NOT NULL;
DROP TABLE __whatsapp_mobile_rollback_guard;
ALTER TABLE device DROP COLUMN mobile_session_binding;
