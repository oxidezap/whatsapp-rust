-- Durable buffer for decrypted inbound messages awaiting an inbound-durability
-- hook commit. The hook gates the transport ack: a row lives here until the
-- hook confirms the message is committed, so a crash before the commit replays
-- the message on the next connect instead of losing it.
--
-- Keyed by (chat, sender, id): stanza ids are only unique within a (chat,
-- sender), matching how msg_secrets and the retry cache scope message ids. A
-- bare id key would let a same-id message in another chat clobber a still
-- pending buffer.

CREATE TABLE pending_inbound_messages (
    chat TEXT NOT NULL,
    sender TEXT NOT NULL,
    id TEXT NOT NULL,
    message BLOB NOT NULL,
    device_id INTEGER NOT NULL DEFAULT 1,
    inserted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (chat, sender, id, device_id)
);

-- device_id first (equality) then inserted_at (range) for the retention sweep.
CREATE INDEX idx_pending_inbound_inserted ON pending_inbound_messages (device_id, inserted_at);
