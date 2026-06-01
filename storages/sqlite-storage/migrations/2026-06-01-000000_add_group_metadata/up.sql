-- Persisted group metadata for the participant-phash re-query optimization
-- (WA Web GroupParticipant.computeGroupParticipantsHash + queryGroup phash).
-- `info` is an opaque, caller-serialized GroupInfo snapshot. On a cache miss we
-- send a phash computed from it so the server can answer "not-modified" by
-- omitting <group> instead of returning the full metadata.
CREATE TABLE group_metadata (
    group_jid TEXT NOT NULL,
    info BLOB NOT NULL,
    device_id INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (group_jid, device_id)
);
