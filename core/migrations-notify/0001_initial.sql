CREATE TABLE IF NOT EXISTS sent (
  topic   TEXT NOT NULL,
  key     TEXT NOT NULL,
  sent_at INTEGER NOT NULL,
  PRIMARY KEY (topic, key)
);

CREATE INDEX IF NOT EXISTS idx_sent_at ON sent(sent_at);
