ALTER TABLE entries
    ADD COLUMN provenance TEXT NOT NULL DEFAULT 'legacy_only';
ALTER TABLE entries
    ADD COLUMN legacy_media_url TEXT;
ALTER TABLE entries
    ADD COLUMN alt TEXT;
ALTER TABLE entries
    ADD COLUMN authors TEXT;

CREATE INDEX IF NOT EXISTS idx_entries_provenance ON entries (provenance);

CREATE TABLE IF NOT EXISTS divergences
(
    date_id      INTEGER NOT NULL,
    field        TEXT    NOT NULL,
    legacy_value TEXT,
    modern_value TEXT,
    PRIMARY KEY (date_id, field)
);

CREATE INDEX IF NOT EXISTS idx_divergences_field ON divergences (field);
