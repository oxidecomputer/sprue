-- Remove the expires_at column from server_registration table
ALTER TABLE server_registration DROP COLUMN IF EXISTS expires_at;

-- Drop the index on expires_at
DROP INDEX IF EXISTS idx_server_registration_expires_at;