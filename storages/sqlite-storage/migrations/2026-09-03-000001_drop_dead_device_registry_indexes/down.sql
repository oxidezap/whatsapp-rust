CREATE INDEX idx_device_registry_timestamp ON device_registry (timestamp);
CREATE INDEX idx_device_registry_device ON device_registry (device_id);
CREATE INDEX idx_device_registry_updated_at ON device_registry (updated_at);
