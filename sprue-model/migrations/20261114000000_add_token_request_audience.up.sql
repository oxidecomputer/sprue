-- Record the audience of the token that each request issues. Requests made
-- before this column existed have no recorded audience.
ALTER TABLE token_request ADD COLUMN audience TEXT;
