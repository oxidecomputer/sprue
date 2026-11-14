ALTER TABLE server_registration
    ADD COLUMN nonce TEXT,
    ADD COLUMN expires_at TIMESTAMPTZ;
CREATE INDEX idx_server_registration_expires_at ON server_registration(expires_at) WHERE expires_at IS NOT NULL;

DROP INDEX IF EXISTS idx_registration_challenge_expires_at;
DROP TABLE IF EXISTS registration_challenge;
