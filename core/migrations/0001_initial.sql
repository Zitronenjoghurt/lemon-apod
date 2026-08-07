CREATE TABLE IF NOT EXISTS entries (
  date_id           INTEGER PRIMARY KEY,   -- days since 1995-06-16
  date              TEXT NOT NULL UNIQUE,  -- 'YYYY-MM-DD'
  title             TEXT NOT NULL,
  title_raw         TEXT,
  explanation_html  TEXT NOT NULL,
  explanation_text  TEXT NOT NULL,
  credits           TEXT,                  -- JSON array of {role, html, text}
  credit_text       TEXT,                  -- every credited name, derived, so FTS can index it
  has_copyright     INTEGER NOT NULL DEFAULT 0,
  license_url       TEXT,
  tomorrow_teaser   TEXT,
  keywords          TEXT,                  -- JSON array
  media_kind        TEXT NOT NULL,
  media_url         TEXT,
  media_hd_url      TEXT,
  thumb_path        TEXT,                  -- 'YYYY/MM/YYYY-MM-DD.webp', relative to the root
  source_url        TEXT NOT NULL,
  parser_version    INTEGER NOT NULL,
  parsed_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entries_kind ON entries(media_kind);
CREATE INDEX IF NOT EXISTS idx_entries_parser ON entries(parser_version);

-- Multi-image entries. The primary media stays on `entries`.
CREATE TABLE IF NOT EXISTS entry_media (
  date_id INTEGER NOT NULL REFERENCES entries(date_id) ON DELETE CASCADE,
  idx     INTEGER NOT NULL,
  kind    TEXT NOT NULL,
  url     TEXT,
  hd_url  TEXT,
  PRIMARY KEY (date_id, idx)
);

CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
  title, explanation_text, credit_text, keywords,
  content = 'entries', content_rowid = 'date_id',
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
  INSERT INTO entries_fts(rowid, title, explanation_text, credit_text, keywords)
  VALUES (new.date_id, new.title, new.explanation_text, new.credit_text, new.keywords);
END;

CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, title, explanation_text, credit_text, keywords)
  VALUES ('delete', old.date_id, old.title, old.explanation_text, old.credit_text, old.keywords);
END;

CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
  INSERT INTO entries_fts(entries_fts, rowid, title, explanation_text, credit_text, keywords)
  VALUES ('delete', old.date_id, old.title, old.explanation_text, old.credit_text, old.keywords);
  INSERT INTO entries_fts(rowid, title, explanation_text, credit_text, keywords)
  VALUES (new.date_id, new.title, new.explanation_text, new.credit_text, new.keywords);
END;

CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
