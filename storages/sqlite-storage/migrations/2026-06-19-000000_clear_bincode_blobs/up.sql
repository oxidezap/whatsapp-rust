-- These BLOBs were bincode-encoded; the storage layer now reads/writes them as
-- protobuf, which cannot decode the old bytes. Clear them so nothing undecodable
-- is left behind: app-state sync keys are re-requested from the primary device,
-- app-state versions re-sync from 0, and the server cert chain is rebuilt on the
-- next handshake.
DELETE FROM app_state_keys;
DELETE FROM app_state_versions;
UPDATE device SET server_cert_chain = NULL;
