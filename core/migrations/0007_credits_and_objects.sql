CREATE TABLE IF NOT EXISTS entry_credits
(
    date_id     INTEGER NOT NULL REFERENCES entries (date_id) ON DELETE CASCADE,
    contributor TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    role        TEXT    NOT NULL,
    url         TEXT,
    kind        TEXT    NOT NULL,
    PRIMARY KEY (date_id, contributor)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_entry_credits_contributor ON entry_credits (contributor);
CREATE INDEX IF NOT EXISTS idx_entry_credits_kind ON entry_credits (kind);

CREATE TABLE IF NOT EXISTS entry_objects
(
    date_id  INTEGER NOT NULL REFERENCES entries (date_id) ON DELETE CASCADE,
    object   TEXT    NOT NULL,
    name     TEXT    NOT NULL,
    catalog  TEXT    NOT NULL,
    in_title         INTEGER NOT NULL DEFAULT 0,
    in_keywords      INTEGER NOT NULL DEFAULT 0,
    in_explanation   INTEGER NOT NULL DEFAULT 0,
    explanation_hits INTEGER NOT NULL DEFAULT 0,
    score            INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (date_id, object)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_entry_objects_object ON entry_objects (object, score);
