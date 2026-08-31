-- `@c.us` and `@s.whatsapp.net` are two spellings of one namespace, and the
-- client now resolves the first to the second while parsing, so nothing can
-- write the legacy spelling again. Rows written before that point are keyed by
-- a string that will never be looked up again: string comparison in SQLite does
-- not go through Rust's `PartialEq`, so a `@c.us` key is simply a different key.
--
-- Only columns holding a rendered JID are rewritten, anchored on the suffix.
-- The Signal address columns (`sessions`, `identities`, `sender_keys`,
-- `base_keys`) are deliberately left alone: `@c.us` is the correct and current
-- spelling there, matching WA Web, and rewriting them would invalidate every
-- established session.
--
-- `UPDATE OR REPLACE` because these columns are primary keys: where both
-- spellings of the same peer exist, the rows collide, and the surviving one is
-- the row being rewritten. Everything touched here is a cache or a resendable
-- payload, so losing the older duplicate costs at most one refetch.
UPDATE OR REPLACE device_registry
   SET user_id = substr(user_id, 1, length(user_id) - 5) || '@s.whatsapp.net'
 WHERE user_id LIKE '%@c.us';

UPDATE OR REPLACE tc_tokens
   SET jid = substr(jid, 1, length(jid) - 5) || '@s.whatsapp.net'
 WHERE jid LIKE '%@c.us';

UPDATE OR REPLACE sent_messages
   SET chat_jid = substr(chat_jid, 1, length(chat_jid) - 5) || '@s.whatsapp.net'
 WHERE chat_jid LIKE '%@c.us';

UPDATE OR REPLACE sender_key_devices
   SET device_jid = substr(device_jid, 1, length(device_jid) - 5) || '@s.whatsapp.net'
 WHERE device_jid LIKE '%@c.us';

UPDATE OR REPLACE msg_secrets
   SET chat = substr(chat, 1, length(chat) - 5) || '@s.whatsapp.net'
 WHERE chat LIKE '%@c.us';

UPDATE OR REPLACE msg_secrets
   SET sender = substr(sender, 1, length(sender) - 5) || '@s.whatsapp.net'
 WHERE sender LIKE '%@c.us';

UPDATE OR REPLACE pending_inbound_messages
   SET chat = substr(chat, 1, length(chat) - 5) || '@s.whatsapp.net'
 WHERE chat LIKE '%@c.us';

UPDATE OR REPLACE pending_inbound_messages
   SET sender = substr(sender, 1, length(sender) - 5) || '@s.whatsapp.net'
 WHERE sender LIKE '%@c.us';

UPDATE OR REPLACE device
   SET pn = substr(pn, 1, length(pn) - 5) || '@s.whatsapp.net'
 WHERE pn LIKE '%@c.us';
