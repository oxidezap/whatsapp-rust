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
-- Where both spellings of one peer exist the rows collide on the primary key,
-- and which one survives matters. For the three caches below it decides what
-- the client believes about a peer, so the collision is resolved by freshness
-- first: the loser is deleted, then the rewrite cannot collide at all. Letting
-- `UPDATE OR REPLACE` pick would always keep the row being rewritten -- the
-- legacy one, which is by construction the older of the two -- and for
-- `device_registry` that means a stale device list replacing a current one,
-- which `get_devices` would then serve until the next refresh, dropping linked
-- devices from every send in between.

-- device_registry: the cached device list per peer.
DELETE FROM device_registry
 WHERE user_id LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM device_registry AS canon
        WHERE canon.user_id =
              substr(device_registry.user_id, 1, length(device_registry.user_id) - 5) || '@s.whatsapp.net'
          AND canon.device_id = device_registry.device_id
          AND canon.updated_at >= device_registry.updated_at);

DELETE FROM device_registry
 WHERE user_id NOT LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM device_registry AS legacy
        WHERE legacy.user_id LIKE '%@c.us'
          AND substr(legacy.user_id, 1, length(legacy.user_id) - 5) || '@s.whatsapp.net' = device_registry.user_id
          AND legacy.device_id = device_registry.device_id);

UPDATE device_registry
   SET user_id = substr(user_id, 1, length(user_id) - 5) || '@s.whatsapp.net'
 WHERE user_id LIKE '%@c.us';

-- tc_tokens: the per-peer trusted-contact token.
DELETE FROM tc_tokens
 WHERE jid LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM tc_tokens AS canon
        WHERE canon.jid = substr(tc_tokens.jid, 1, length(tc_tokens.jid) - 5) || '@s.whatsapp.net'
          AND canon.device_id = tc_tokens.device_id
          AND canon.updated_at >= tc_tokens.updated_at);

DELETE FROM tc_tokens
 WHERE jid NOT LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM tc_tokens AS legacy
        WHERE legacy.jid LIKE '%@c.us'
          AND substr(legacy.jid, 1, length(legacy.jid) - 5) || '@s.whatsapp.net' = tc_tokens.jid
          AND legacy.device_id = tc_tokens.device_id);

UPDATE tc_tokens
   SET jid = substr(jid, 1, length(jid) - 5) || '@s.whatsapp.net'
 WHERE jid LIKE '%@c.us';

-- sender_key_devices: which devices hold the current sender key for a group.
DELETE FROM sender_key_devices
 WHERE device_jid LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM sender_key_devices AS canon
        WHERE canon.device_jid =
              substr(sender_key_devices.device_jid, 1, length(sender_key_devices.device_jid) - 5) || '@s.whatsapp.net'
          AND canon.group_jid = sender_key_devices.group_jid
          AND canon.device_id = sender_key_devices.device_id
          AND canon.updated_at >= sender_key_devices.updated_at);

DELETE FROM sender_key_devices
 WHERE device_jid NOT LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM sender_key_devices AS legacy
        WHERE legacy.device_jid LIKE '%@c.us'
          AND substr(legacy.device_jid, 1, length(legacy.device_jid) - 5) || '@s.whatsapp.net' = sender_key_devices.device_jid
          AND legacy.group_jid = sender_key_devices.group_jid
          AND legacy.device_id = sender_key_devices.device_id);

UPDATE sender_key_devices
   SET device_jid = substr(device_jid, 1, length(device_jid) - 5) || '@s.whatsapp.net'
 WHERE device_jid LIKE '%@c.us';

-- The rest key on a message id as well as the chat, so a collision means the
-- same message stored under both spellings -- the same bytes either way, with
-- nothing to choose between them. `OR REPLACE` is enough, and is also the
-- backstop that keeps a collision from aborting the migration and leaving the
-- database unopenable.
UPDATE OR REPLACE sent_messages
   SET chat_jid = substr(chat_jid, 1, length(chat_jid) - 5) || '@s.whatsapp.net'
 WHERE chat_jid LIKE '%@c.us';

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

-- `device.pn` is not a key, so it cannot collide with anything.
UPDATE device
   SET pn = substr(pn, 1, length(pn) - 5) || '@s.whatsapp.net'
 WHERE pn LIKE '%@c.us';
