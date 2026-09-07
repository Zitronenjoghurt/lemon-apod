CREATE TABLE IF NOT EXISTS favorites (
  user_id    INTEGER NOT NULL,
  date_id    INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, date_id)
);

CREATE INDEX IF NOT EXISTS idx_favorites_date ON favorites(date_id);
