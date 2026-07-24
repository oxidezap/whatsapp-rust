-- Fold rows written under a device-suffixed JID (`user:48@lid`) onto the bare
-- identity every lookup uses.
--
-- Receipts were the one event that reached the store with the peer's wire
-- identity intact, so a companion device's traffic was filed under keys
-- nothing reads: phantom chat rows, one read-by row per device instead of per
-- participant, and contact names the bare lookup never finds. The writers
-- normalize now; this heals what they left behind. A chat, contact or
-- participant key never legitimately carries a device, so `LIKE '%:%@%'`
-- selects exactly those artifacts.

-- Phantom chats. Only a receipt could key a chat by device and messages always
-- landed on the bare thread, so these are empty; the guard keeps a row that
-- somehow owns messages rather than orphaning them.
DELETE FROM chats
 WHERE jid LIKE '%:%@%'
   AND NOT EXISTS (
     SELECT 1 FROM messages
      WHERE messages.device_id = chats.device_id
        AND messages.chat_jid = chats.jid);

-- Contacts. An existing bare row wins (a live path wrote it with the same or
-- newer data); otherwise the device-keyed names carry over to it.
INSERT OR IGNORE INTO contacts (device_id, jid, push_name, full_name, first_name, business_name)
SELECT device_id,
       substr(jid, 1, instr(jid, ':') - 1) || substr(jid, instr(jid, '@')),
       push_name,
       full_name,
       first_name,
       business_name
  FROM contacts
 WHERE jid LIKE '%:%@%';

DELETE FROM contacts WHERE jid LIKE '%:%@%';

-- Read-by rows. Highest receipt type per participant wins, the live path's
-- monotonic rule: drop every row a sibling of the same bare identity beats,
-- then rename what survives (UPDATE OR IGNORE leaves same-type ties behind,
-- which the final DELETE clears).
DELETE FROM message_receipts
 WHERE EXISTS (
   SELECT 1 FROM message_receipts s
    WHERE s.device_id = message_receipts.device_id
      AND s.chat_jid = message_receipts.chat_jid
      AND s.msg_id = message_receipts.msg_id
      AND s.receipt_type > message_receipts.receipt_type
      AND CASE
            WHEN instr(s.user_jid, ':') > 0
            THEN substr(s.user_jid, 1, instr(s.user_jid, ':') - 1)
                 || substr(s.user_jid, instr(s.user_jid, '@'))
            ELSE s.user_jid
          END = CASE
            WHEN instr(message_receipts.user_jid, ':') > 0
            THEN substr(message_receipts.user_jid, 1, instr(message_receipts.user_jid, ':') - 1)
                 || substr(message_receipts.user_jid, instr(message_receipts.user_jid, '@'))
            ELSE message_receipts.user_jid
          END);

UPDATE OR IGNORE message_receipts
   SET user_jid = substr(user_jid, 1, instr(user_jid, ':') - 1)
                  || substr(user_jid, instr(user_jid, '@'))
 WHERE user_jid LIKE '%:%@%';

DELETE FROM message_receipts WHERE user_jid LIKE '%:%@%';
