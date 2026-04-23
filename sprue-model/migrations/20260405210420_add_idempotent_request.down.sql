-- Drop idempotent_request_state table and indexes
DROP INDEX IF EXISTS idx_idempotent_request_state_state;
DROP INDEX IF EXISTS idx_idempotent_request_state_created_at;
DROP INDEX IF EXISTS idx_idempotent_request_state_request_id;
DROP TABLE IF EXISTS idempotent_request_state;

-- Drop idempotent_request table and indexes
DROP INDEX IF EXISTS idx_idempotent_request_expires_at;
DROP INDEX IF EXISTS idx_idempotent_request_key;
DROP INDEX IF EXISTS idx_idempotent_request_server_registration_id;
DROP TABLE IF EXISTS idempotent_request;