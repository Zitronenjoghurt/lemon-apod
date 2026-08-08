CREATE TABLE IF NOT EXISTS launches (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  provider      TEXT,
  vehicle       TEXT,
  pad           TEXT,
  mission       TEXT,
  orbit         TEXT,
  status        TEXT,
  net           INTEGER NOT NULL,
  window_start  INTEGER,
  window_end    INTEGER,
  precision     TEXT,
  image_url     TEXT,
  info_url      TEXT,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_launches_net ON launches(net);

CREATE TABLE IF NOT EXISTS space_weather (
  id          INTEGER PRIMARY KEY CHECK (id = 1),
  kp          REAL NOT NULL,
  observed_at INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS feeds (
  name        TEXT PRIMARY KEY,
  fetched_at  INTEGER,
  succeeded   INTEGER NOT NULL DEFAULT 0,
  error       TEXT
);
