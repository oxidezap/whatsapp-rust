-- Keep one receipt row per state, not one per user that overwrites itself.
--
-- The old key ended at `user_jid`, so a user's row advanced in place: the
-- `read` receipt overwrote the `delivered` one and took its `ts_ms` with it.
-- That is enough to render a group's "read by" list, which only ever asks for
-- each member's furthest state, and it is why the narrower key held up until
-- 1:1 message info needed something else.
--
-- WA Web models a message's receipts as three separate collections
-- (`.delivery`, `.read`, `.played`), each entry carrying its own `t`, and its
-- 1:1 info drawer reads all three at once to show "Delivered hh:mm" above
-- "Read hh:mm". Both of those instants have to survive for that to render, so
-- the state joins the key and each transition keeps its own row.
--
-- Existing rows are already unique on the narrower key, so they stay unique
-- under the wider one and carry over untouched — each user keeping the single
-- furthest state recorded so far, with earlier states simply absent rather
-- than wrong.
CREATE TABLE message_receipts_new (
    device_id INTEGER NOT NULL,
    chat_jid TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    user_jid TEXT NOT NULL,
    -- Same scale as messages.status: 3 delivered, 4 read, 5 played.
    receipt_type INTEGER NOT NULL,
    ts_ms BIGINT NOT NULL,
    PRIMARY KEY (device_id, chat_jid, msg_id, user_jid, receipt_type)
);

INSERT INTO message_receipts_new
    (device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms)
SELECT device_id, chat_jid, msg_id, user_jid, receipt_type, ts_ms
FROM message_receipts;

DROP TABLE message_receipts;

ALTER TABLE message_receipts_new RENAME TO message_receipts;
