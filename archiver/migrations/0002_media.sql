CREATE TABLE IF NOT EXISTS media (
  url             TEXT PRIMARY KEY,
  date_id         INTEGER NOT NULL,
  role            TEXT NOT NULL,
  source          TEXT NOT NULL,
  path            TEXT,
  http_status     INTEGER,
  sha256          TEXT,
  bytes           INTEGER,
  content_type    TEXT,
  fetched_at      INTEGER,
  last_checked_at INTEGER,
  attempts        INTEGER NOT NULL DEFAULT 0,
  error           TEXT
);

CREATE INDEX IF NOT EXISTS idx_media_date ON media(date_id);
CREATE INDEX IF NOT EXISTS idx_media_source_status ON media(source, http_status);
CREATE INDEX IF NOT EXISTS idx_media_checked ON media(last_checked_at);
