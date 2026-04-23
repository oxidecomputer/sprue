-- Remove index and column
DROP INDEX IF EXISTS idx_blob_server_registration_id;
ALTER TABLE blob DROP COLUMN server_registration_id;
