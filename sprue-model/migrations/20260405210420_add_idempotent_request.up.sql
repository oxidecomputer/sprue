-- Create idempotent_request table to store idempotent request responses
CREATE TABLE IF NOT EXISTS idempotent_request (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_registration_id UUID NOT NULL REFERENCES server_registration(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    response JSONB,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    -- Ensure uniqueness of idempotency key per server registration
    CONSTRAINT idempotent_request_unique_key UNIQUE (server_registration_id, idempotency_key)
);

-- Create index on server_registration_id for efficient lookups
CREATE INDEX idx_idempotent_request_server_registration_id ON idempotent_request(server_registration_id);

-- Create index on idempotency_key for efficient lookups
CREATE INDEX idx_idempotent_request_idempotency_key ON idempotent_request(idempotency_key);

-- Create index on expires_at for efficient cleanup of expired requests
CREATE INDEX idx_idempotent_request_expires_at ON idempotent_request(expires_at);

-- Create idempotent_request_state table to track all state transitions
CREATE TABLE IF NOT EXISTS idempotent_request_state (
    id BIGSERIAL PRIMARY KEY,
    idempotent_request_id UUID NOT NULL REFERENCES idempotent_request(id) ON DELETE CASCADE,
    state JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Create index on idempotent_request_id for efficient lookups
CREATE INDEX idx_idempotent_request_state_request_id ON idempotent_request_state(idempotent_request_id);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_idempotent_request_state_created_at ON idempotent_request_state(idempotent_request_id, created_at DESC);

-- Create index on state for efficient filtering
CREATE INDEX idx_idempotent_request_state_state ON idempotent_request_state USING GIN(state);