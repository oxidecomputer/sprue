-- A registration challenge is issued to a VM that wants to join a service. The
-- VM answers it with an attestation, and only then is a server registration
-- created from the attested instance identity.
CREATE TABLE IF NOT EXISTS registration_challenge (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES service(id) ON DELETE CASCADE,
    nonce TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_registration_challenge_expires_at ON registration_challenge(expires_at);

-- Registrations that were never proven hold an instance, project and silo that
-- nothing has verified. They can no longer be proven, so remove them.
DELETE FROM server_registration sr
WHERE (
    SELECT srs.state
    FROM server_registration_state srs
    WHERE srs.server_registration_id = sr.id
    ORDER BY srs.created_at DESC
    LIMIT 1
) = '"Pending"'::jsonb;

-- Registrations no longer carry their own challenge.
DROP INDEX IF EXISTS idx_server_registration_expires_at;
ALTER TABLE server_registration
    DROP COLUMN IF EXISTS nonce,
    DROP COLUMN IF EXISTS expires_at;
