CREATE TABLE fetches_new (
  date_id         INTEGER NOT NULL,
  source          TEXT    NOT NULL,
  url             TEXT    NOT NULL,
  final_url       TEXT,
  http_status     INTEGER,
  sha256          TEXT,
  bytes           INTEGER,
  fetched_at      INTEGER,
  last_checked_at INTEGER,
  attempts        INTEGER NOT NULL DEFAULT 0,
  error           TEXT,
  PRIMARY KEY (date_id, source)
);

INSERT INTO fetches_new (date_id, source, url, http_status, sha256, bytes, fetched_at,
                         last_checked_at, attempts, error)
SELECT date_id, 'legacy', url, http_status, sha256, bytes, fetched_at, last_checked_at,
       CASE WHEN http_status = 200 THEN 0 ELSE 1 END, error
FROM fetches;

DROP TABLE fetches;

ALTER TABLE fetches_new RENAME TO fetches;

CREATE INDEX IF NOT EXISTS idx_fetches_checked ON fetches(last_checked_at);
CREATE INDEX IF NOT EXISTS idx_fetches_source_status ON fetches(source, http_status);
