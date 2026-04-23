-- Revert instance_id (UUID) back to public_key (TEXT)

-- Drop the new index
DROP INDEX IF EXISTS idx_server_registration_instance_id;

-- Rename column and change type back to TEXT
ALTER TABLE server_registration
    ALTER COLUMN instance_id TYPE TEXT USING instance_id::TEXT,
    RENAME COLUMN instance_id TO public_key;

-- Recreate the original index
CREATE INDEX idx_server_registration_public_key ON server_registration(public_key);