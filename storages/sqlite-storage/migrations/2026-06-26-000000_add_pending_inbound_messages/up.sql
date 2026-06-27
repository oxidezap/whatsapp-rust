-- Durable buffer for decrypted inbound messages awaiting an inbound-durability
-- hook commit. The hook gates the transport ack: a row lives here until the
-- hook confirms the message is committed, so a crash before the commit replays
-- the message on the next connect instead of losing it. Keyed by the stanza id.

CREATE TABLE pending_inbound_messages (
    id TEXT NOT NULL,
    message BLOB NOT NULL,
    device_id INTEGER NOT NULL DEFAULT 1,
    inserted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (id, device_id)
);

CREATE INDEX idx_pending_inbound_inserted ON pending_inbound_messages (inserted_at, device_id);
