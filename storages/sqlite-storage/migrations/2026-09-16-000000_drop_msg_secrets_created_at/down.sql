-- Re-add `created_at` with a default. The original values are unrecoverable
-- (dropping a column rewrites the rows), but nothing reads the column on any
-- current path: the 2026-05-31 backfill consumed it, and a rollback past that
-- migration regenerates it from `expires_at` anyway.
ALTER TABLE msg_secrets ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
