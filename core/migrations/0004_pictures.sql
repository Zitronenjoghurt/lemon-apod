ALTER TABLE entries
    ADD COLUMN phash BLOB;
ALTER TABLE entries
    ADD COLUMN picture_group INTEGER;

CREATE INDEX IF NOT EXISTS idx_entries_picture_group ON entries (picture_group);
CREATE INDEX IF NOT EXISTS idx_entries_playable ON entries (media_kind, thumb_path);
