-- Create deployment table to associate services with project/silo pairs
CREATE TABLE IF NOT EXISTS deployment (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES service(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    silo_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (service_id, project_id, silo_id)
);

-- Create index on service_id for efficient lookups
CREATE INDEX idx_deployment_service_id ON deployment(service_id);

-- Create composite index for lookups by project and silo
CREATE INDEX idx_deployment_project_silo ON deployment(project_id, silo_id);
