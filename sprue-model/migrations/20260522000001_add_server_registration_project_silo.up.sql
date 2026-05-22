ALTER TABLE server_registration
    ADD COLUMN project_id UUID NOT NULL,
    ADD COLUMN silo_id UUID NOT NULL;

CREATE INDEX idx_server_registration_project_silo ON server_registration(project_id, silo_id);
