-- Create token_request table (mirrors server_registration structure)
CREATE TABLE IF NOT EXISTS token_request (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_registration_id UUID NOT NULL REFERENCES server_registration(id) ON DELETE CASCADE,
    nonce TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Create index on server_registration_id for efficient lookups
CREATE INDEX idx_token_request_server_registration_id ON token_request(server_registration_id);

-- Create index on expires_at for efficient expiration queries
CREATE INDEX idx_token_request_expires_at ON token_request(expires_at) WHERE expires_at IS NOT NULL;

-- Create index on created_at for efficient sorting
CREATE INDEX idx_token_request_created_at ON token_request(created_at DESC);

-- Create token_request_state table to track all state transitions
CREATE TABLE IF NOT EXISTS token_request_state (
    id BIGSERIAL PRIMARY KEY,
    token_request_id UUID NOT NULL REFERENCES token_request(id) ON DELETE CASCADE,
    state JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Create index on token_request_id for efficient lookups
CREATE INDEX idx_token_request_state_request_id ON token_request_state(token_request_id);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_token_request_state_created_at ON token_request_state(token_request_id, created_at DESC);

-- Create index on state for efficient filtering
CREATE INDEX idx_token_request_state_state ON token_request_state USING GIN(state);