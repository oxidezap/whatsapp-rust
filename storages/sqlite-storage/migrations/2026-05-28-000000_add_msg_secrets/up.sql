-- MessageContextInfo.messageSecret persistence keyed by the outbound message.
-- Required to decrypt msmsg replies from Meta AI / bot fbid: WAWebBotMessageSecret
-- looks up the secret by (chat, target_sender, target_id) where target_id is the
-- id of our original outbound message.

CREATE TABLE msg_secrets (
    chat TEXT NOT NULL,
    sender TEXT NOT NULL,
    msg_id TEXT NOT NULL,
    secret BLOB NOT NULL,
    device_id INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (chat, sender, msg_id, device_id)
);

CREATE INDEX idx_msg_secrets_created ON msg_secrets (created_at, device_id);
