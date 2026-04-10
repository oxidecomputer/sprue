-- Enable pgcrypto extension for gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create service table
CREATE TABLE IF NOT EXISTS service (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Create index on name for efficient lookups
CREATE INDEX idx_service_name ON service(name);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_service_created_at ON service(created_at);

-- Create server_registration table to store service public keys
CREATE TABLE IF NOT EXISTS server_registration (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES service(id) ON DELETE CASCADE,
    public_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Create index on service_id for efficient lookups
CREATE INDEX idx_server_registration_service_id ON server_registration(service_id);

-- Create index on public_key for efficient lookups
CREATE INDEX idx_server_registration_public_key ON server_registration(public_key);

-- Create server_registration_state table to track all state transitions
CREATE TABLE IF NOT EXISTS server_registration_state (
    id BIGSERIAL PRIMARY KEY,
    server_registration_id UUID NOT NULL REFERENCES server_registration(id) ON DELETE CASCADE,
    state JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Create index on server_registration_id for efficient lookups
CREATE INDEX idx_server_registration_state_registration_id ON server_registration_state(server_registration_id);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_server_registration_state_created_at ON server_registration_state(server_registration_id, created_at DESC);

-- Create index on state for efficient filtering
CREATE INDEX idx_server_registration_state_state ON server_registration_state USING GIN(state);

-- Create backup table
CREATE TABLE IF NOT EXISTS backup (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES service(id) ON DELETE CASCADE,
    backup_time TIMESTAMPTZ NOT NULL,
    size BIGINT NOT NULL,
    total_size BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Create index on service_id for efficient lookups
CREATE INDEX idx_backup_service_id ON backup(service_id);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_backup_created_at ON backup(created_at DESC);

-- Create backup_state table to track all state transitions
CREATE TABLE IF NOT EXISTS backup_state (
    id BIGSERIAL PRIMARY KEY,
    backup_id UUID NOT NULL REFERENCES backup(id) ON DELETE CASCADE,
    state JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Create index on backup_id for efficient lookups
CREATE INDEX idx_backup_state_backup_id ON backup_state(backup_id);

-- Create index on created_at for efficient sorting
CREATE INDEX idx_backup_state_created_at ON backup_state(backup_id, created_at DESC);

-- Create index on state for efficient filtering
CREATE INDEX idx_backup_state_state ON backup_state USING GIN(state);

-- Create health_check table to track server registration health check-ins
CREATE TABLE IF NOT EXISTS health_check (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_registration_id UUID NOT NULL REFERENCES server_registration(id) ON DELETE CASCADE,
    checked_in_at TIMESTAMPTZ NOT NULL
);

-- Create index on server_registration_id for efficient lookups
CREATE INDEX idx_health_check_server_registration_id ON health_check(server_registration_id);

-- Create index on checked_in_at for efficient time-based queries
CREATE INDEX idx_health_check_checked_in_at ON health_check(server_registration_id, checked_in_at DESC);