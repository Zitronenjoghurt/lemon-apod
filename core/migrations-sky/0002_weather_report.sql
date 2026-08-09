CREATE TABLE IF NOT EXISTS weather_report (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  body       TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
