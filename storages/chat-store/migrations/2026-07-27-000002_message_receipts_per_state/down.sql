-- Back to one row per user. Several states per user collapse to the furthest
-- one, which is what the narrower key could represent; the earlier instants
-- are dropped because there is nowhere to put them.
CREATE TABLE message_receipts_old (
    device_id INTEGER NOT NULL,
    chat_jid TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    user_jid TEXT NOT NULL,
    receipt_type INTEGER NOT NULL,
    ts_ms BIGINT NOT NULL,
    PRIMARY KEY (device_id, chat_jid, msg_id, user_jid)
);

INSERT INTO message_receipts_old
    (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms)
SELECT r.device_id, r.chat_jid, r.msg_id, r.user_jid, r.receipt_type, r.ts_ms
FROM message_receipts r
WHERE r.receipt_type = (
    SELECT MAX(s.receipt_type) FROM message_receipts s
    WHERE s.device_id = r.device_id AND s.chat_jid = r.chat_jid
      AND s.msg_id = r.msg_id AND s.user_jid = r.user_jid
);

DROP TABLE message_receipts;

ALTER TABLE message_receipts_old RENAME TO message_receipts;
