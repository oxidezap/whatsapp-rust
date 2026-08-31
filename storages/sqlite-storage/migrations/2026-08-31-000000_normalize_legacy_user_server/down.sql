-- Not reversible, and not lossy either: the rows this migration rewrote are
-- addressed by the same identity under either spelling, and the client that
-- reads them can no longer produce the old one. Restoring it would recreate
-- keys nothing looks up.
SELECT 1;
