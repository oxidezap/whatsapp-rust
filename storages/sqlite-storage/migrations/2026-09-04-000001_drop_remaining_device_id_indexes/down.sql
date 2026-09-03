CREATE INDEX IF NOT EXISTS idx_signed_prekeys_device_id ON signed_prekeys (device_id);
CREATE INDEX IF NOT EXISTS idx_app_state_keys_device_id ON app_state_keys (device_id);
CREATE INDEX IF NOT EXISTS idx_app_state_versions_device_id ON app_state_versions (device_id);
CREATE INDEX IF NOT EXISTS idx_app_state_mutation_macs_device_id ON app_state_mutation_macs (device_id);
CREATE INDEX IF NOT EXISTS idx_base_keys_device ON base_keys (device_id);
