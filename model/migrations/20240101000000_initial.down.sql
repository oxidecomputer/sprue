-- Drop health_check table (has foreign key to server_registration)
DROP TABLE IF EXISTS health_check;

-- Drop server_registration_state table (has foreign key to server_registration)
DROP TABLE IF EXISTS server_registration_state;

-- Drop server_registration table (has foreign key to service)
DROP TABLE IF EXISTS server_registration;

-- Drop backup_state table (has foreign key to backup)
DROP TABLE IF EXISTS backup_state;

-- Drop backup table (has foreign key to service)
DROP TABLE IF EXISTS backup;

-- Drop service table
DROP TABLE IF EXISTS service;