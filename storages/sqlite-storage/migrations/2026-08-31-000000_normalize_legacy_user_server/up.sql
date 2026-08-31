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
-- One rule decides every collision: **a legacy row never displaces a canonical
-- one.** Where both spellings of the same key exist, the legacy row is deleted
-- and the canonical row is left exactly as it is; only an uncontested legacy
-- row is rewritten.
--
-- That is not a coin toss between two candidates, it is the only choice that
-- cannot lose information. The canonical row is the one the current client has
-- been reading and writing all along, and the legacy row is already unreachable
-- to it -- so deleting the legacy row costs nothing that is live today, while
-- keeping it can only substitute stale state for current state. It is also why
-- no per-table merge is attempted: every table below has its own freshness
-- rule (`put_msg_secrets` keeps the later `expires_at` and a non-zero
-- `message_ts`; `set_sender_key_status` must not let a stale `has_key = true`
-- outlive a forget mark; `tc_tokens` advances `token_timestamp` and
-- `sender_timestamp` independently), and a migration that tried to reproduce
-- each of them would be a second implementation of every writer, drifting from
-- the first. Preferring the canonical row defers to whatever those writers
-- already decided.
--
-- Deleting first also means the rewrite cannot collide, so it needs no
-- `OR REPLACE` and a surprise collision would surface as a failure rather than
-- silently discarding a row.

-- device_registry: the cached device list per peer.
DELETE FROM device_registry
 WHERE user_id LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM device_registry AS canon
        WHERE canon.user_id =
              substr(device_registry.user_id, 1, length(device_registry.user_id) - 5) || '@s.whatsapp.net'
          AND canon.device_id = device_registry.device_id);

UPDATE device_registry
   SET user_id = substr(user_id, 1, length(user_id) - 5) || '@s.whatsapp.net'
 WHERE user_id LIKE '%@c.us';

-- tc_tokens: the per-peer trusted-contact token.
DELETE FROM tc_tokens
 WHERE jid LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM tc_tokens AS canon
        WHERE canon.jid = substr(tc_tokens.jid, 1, length(tc_tokens.jid) - 5) || '@s.whatsapp.net'
          AND canon.device_id = tc_tokens.device_id);

UPDATE tc_tokens
   SET jid = substr(jid, 1, length(jid) - 5) || '@s.whatsapp.net'
 WHERE jid LIKE '%@c.us';

-- sender_key_devices: which devices hold the current sender key for a group.
-- `group_jid` is part of the key, so the counterpart must match on it too: a
-- legacy row for a device in one group must not be deleted because that device
-- has a canonical row in a different group.
DELETE FROM sender_key_devices
 WHERE device_jid LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM sender_key_devices AS canon
        WHERE canon.device_jid =
              substr(sender_key_devices.device_jid, 1, length(sender_key_devices.device_jid) - 5) || '@s.whatsapp.net'
          AND canon.group_jid = sender_key_devices.group_jid
          AND canon.device_id = sender_key_devices.device_id);

UPDATE sender_key_devices
   SET device_jid = substr(device_jid, 1, length(device_jid) - 5) || '@s.whatsapp.net'
 WHERE device_jid LIKE '%@c.us';

-- sent_messages: the retry payload, keyed by chat and message id.
DELETE FROM sent_messages
 WHERE chat_jid LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM sent_messages AS canon
        WHERE canon.chat_jid =
              substr(sent_messages.chat_jid, 1, length(sent_messages.chat_jid) - 5) || '@s.whatsapp.net'
          AND canon.message_id = sent_messages.message_id
          AND canon.device_id = sent_messages.device_id);

UPDATE sent_messages
   SET chat_jid = substr(chat_jid, 1, length(chat_jid) - 5) || '@s.whatsapp.net'
 WHERE chat_jid LIKE '%@c.us';

-- msg_secrets: keyed by chat, sender and message id, so both jid columns move
-- and either can collide. Deleted per column in turn, each against the shape
-- the row would take after that column is rewritten.
DELETE FROM msg_secrets
 WHERE chat LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM msg_secrets AS canon
        WHERE canon.chat = substr(msg_secrets.chat, 1, length(msg_secrets.chat) - 5) || '@s.whatsapp.net'
          AND canon.sender = msg_secrets.sender
          AND canon.msg_id = msg_secrets.msg_id
          AND canon.device_id = msg_secrets.device_id);

UPDATE msg_secrets
   SET chat = substr(chat, 1, length(chat) - 5) || '@s.whatsapp.net'
 WHERE chat LIKE '%@c.us';

DELETE FROM msg_secrets
 WHERE sender LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM msg_secrets AS canon
        WHERE canon.sender = substr(msg_secrets.sender, 1, length(msg_secrets.sender) - 5) || '@s.whatsapp.net'
          AND canon.chat = msg_secrets.chat
          AND canon.msg_id = msg_secrets.msg_id
          AND canon.device_id = msg_secrets.device_id);

UPDATE msg_secrets
   SET sender = substr(sender, 1, length(sender) - 5) || '@s.whatsapp.net'
 WHERE sender LIKE '%@c.us';

-- pending_inbound_messages: the durability buffer, same two-column shape.
DELETE FROM pending_inbound_messages
 WHERE chat LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM pending_inbound_messages AS canon
        WHERE canon.chat =
              substr(pending_inbound_messages.chat, 1, length(pending_inbound_messages.chat) - 5) || '@s.whatsapp.net'
          AND canon.sender = pending_inbound_messages.sender
          AND canon.id = pending_inbound_messages.id
          AND canon.device_id = pending_inbound_messages.device_id);

UPDATE pending_inbound_messages
   SET chat = substr(chat, 1, length(chat) - 5) || '@s.whatsapp.net'
 WHERE chat LIKE '%@c.us';

DELETE FROM pending_inbound_messages
 WHERE sender LIKE '%@c.us'
   AND EXISTS (
       SELECT 1 FROM pending_inbound_messages AS canon
        WHERE canon.sender =
              substr(pending_inbound_messages.sender, 1, length(pending_inbound_messages.sender) - 5) || '@s.whatsapp.net'
          AND canon.chat = pending_inbound_messages.chat
          AND canon.id = pending_inbound_messages.id
          AND canon.device_id = pending_inbound_messages.device_id);

UPDATE pending_inbound_messages
   SET sender = substr(sender, 1, length(sender) - 5) || '@s.whatsapp.net'
 WHERE sender LIKE '%@c.us';

-- `device.pn` is not a key, so it cannot collide with anything.
UPDATE device
   SET pn = substr(pn, 1, length(pn) - 5) || '@s.whatsapp.net'
 WHERE pn LIKE '%@c.us';
