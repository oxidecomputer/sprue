DROP INDEX IF EXISTS idx_server_registration_project_silo;

ALTER TABLE server_registration
    DROP COLUMN IF EXISTS project_id,
    DROP COLUMN IF EXISTS silo_id;
