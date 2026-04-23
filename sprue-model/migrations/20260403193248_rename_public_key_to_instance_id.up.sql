-- Rename public_key column to instance_id and change type from TEXT to UUID
-- First drop the existing index on public_key
DROP INDEX IF EXISTS idx_server_registration_public_key;

-- Add new instance_id column as UUID
ALTER TABLE server_registration ADD COLUMN instance_id UUID;

-- Generate UUIDs for existing rows (if any)
UPDATE server_registration SET instance_id = gen_random_uuid() WHERE instance_id IS NULL;

-- Make instance_id NOT NULL and UNIQUE
ALTER TABLE server_registration ALTER COLUMN instance_id SET NOT NULL;
ALTER TABLE server_registration ADD CONSTRAINT server_registration_instance_id_key UNIQUE (instance_id);

-- Drop the old public_key column
ALTER TABLE server_registration DROP COLUMN public_key;

-- Create index on instance_id for efficient lookups
CREATE INDEX idx_server_registration_instance_id ON server_registration(instance_id);