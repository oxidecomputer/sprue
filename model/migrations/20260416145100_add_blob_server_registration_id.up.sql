-- Add server_registration_id to blob table
ALTER TABLE blob ADD COLUMN server_registration_id UUID NOT NULL REFERENCES server_registration(id) ON DELETE CASCADE;

-- Create index on server_registration_id for efficient lookups
CREATE INDEX idx_blob_server_registration_id ON blob(server_registration_id);