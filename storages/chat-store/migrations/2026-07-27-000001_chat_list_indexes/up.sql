-- Let the chat list stream from an index instead of sorting the whole table.
--
-- `chats()` orders by `(pinned_at IS NULL, pinned_at DESC, last_message_ts
-- DESC)`. SQLite cannot serve a leading `pinned_at IS NULL` expression from a
-- plain column index, so that sort was a full scan of `chats` plus a temp
-- B-tree on every call — including the calls that only wanted one page, or one
-- chat. The reader splits it into a pinned pass and a non-pinned pass now, and
-- these two indexes make each pass a pure ordered range scan.
--
-- Both carry `jid` as the last key column: it completes the primary key, which
-- gives the keyset cursor a unique, stable tiebreak at a page boundary.
--
-- `archived` leaves the key and becomes a filter. As a leading column it split
-- the activity order into two runs, so the archived-inclusive list had to sort
-- them back together; as a filter both list modes stream from one index and
-- stop at LIMIT. It costs a scan past archived chats when they are dense near
-- the head, which is the cheaper side of the trade for a list that is almost
-- always read from the top.
DROP INDEX IF EXISTS idx_chats_order;
CREATE INDEX idx_chats_order ON chats (device_id, last_message_ts DESC, jid DESC);

-- Partial: pinning is capped at a handful of chats, so this stays tiny and
-- costs nothing on the writes that never touch `pinned_at`.
--
-- `last_message_ts` sits between the two because pin times collide in
-- practice: history sync carries them at second precision, so several chats
-- can share one. Activity decides between equally-pinned chats, as it did
-- under the old combined sort; `jid` only settles what activity cannot.
CREATE INDEX idx_chats_pinned ON chats (device_id, pinned_at DESC, last_message_ts DESC, jid DESC)
    WHERE pinned_at IS NOT NULL;
