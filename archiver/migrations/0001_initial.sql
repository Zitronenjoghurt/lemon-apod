CREATE TABLE IF NOT EXISTS fetches (
  date_id         INTEGER PRIMARY KEY,
  url             TEXT NOT NULL,
  http_status     INTEGER,
  sha256          TEXT,
  bytes           INTEGER,
  fetched_at      INTEGER,
  last_checked_at INTEGER,
  error           TEXT
);

CREATE INDEX IF NOT EXISTS idx_fetches_checked ON fetches(last_checked_at);

CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
