ALTER TABLE entries
    ADD COLUMN first_stored_at INTEGER;

-- Everything already here settled long ago. parsed_at is the closest record of when it turned
-- up, and it stops the whole archive from reading as if it had arrived this second.
UPDATE entries
SET first_stored_at = parsed_at
WHERE first_stored_at IS NULL;
