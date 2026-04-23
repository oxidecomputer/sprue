-- Drop health_check table (has foreign key to server_registration)
DROP TABLE IF EXISTS health_check;

-- Drop server_registration_state table (has foreign key to server_registration)
DROP TABLE IF EXISTS server_registration_state;

-- Drop server_registration table (has foreign key to service)
DROP TABLE IF EXISTS server_registration;

-- Drop blob_state table (has foreign key to blob)
DROP TABLE IF EXISTS blob_state;

-- Drop blob table (has foreign key to service)
DROP TABLE IF EXISTS blob;

-- Drop service table
DROP TABLE IF EXISTS service;