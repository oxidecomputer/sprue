-- Add optional expires_at column to server_registration table
ALTER TABLE server_registration ADD COLUMN expires_at TIMESTAMPTZ;

-- Create index on expires_at for efficient expiration queries
CREATE INDEX idx_server_registration_expires_at ON server_registration(expires_at) WHERE expires_at IS NOT NULL;