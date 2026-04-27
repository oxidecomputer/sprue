-- Add optional nonce column to server_registration table
ALTER TABLE server_registration ADD COLUMN nonce TEXT;