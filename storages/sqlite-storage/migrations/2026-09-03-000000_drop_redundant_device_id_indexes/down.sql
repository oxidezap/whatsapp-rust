CREATE INDEX IF NOT EXISTS idx_identities_device_id ON identities (device_id);
CREATE INDEX IF NOT EXISTS idx_sessions_device_id ON sessions (device_id);
CREATE INDEX IF NOT EXISTS idx_prekeys_device_id ON prekeys (device_id);
CREATE INDEX IF NOT EXISTS idx_sender_keys_device_id ON sender_keys (device_id);
