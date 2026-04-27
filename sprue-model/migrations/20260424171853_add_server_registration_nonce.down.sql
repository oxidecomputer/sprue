-- Remove the nonce column from server_registration table
ALTER TABLE server_registration DROP COLUMN IF EXISTS nonce;