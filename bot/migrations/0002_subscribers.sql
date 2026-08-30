CREATE TABLE IF NOT EXISTS users (
  user_id      INTEGER PRIMARY KEY,
  explanation  TEXT    NOT NULL DEFAULT 'full',
  enabled      INTEGER NOT NULL DEFAULT 0,
  last_date_id INTEGER,
  updated_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_announcing ON users(enabled, last_date_id);
