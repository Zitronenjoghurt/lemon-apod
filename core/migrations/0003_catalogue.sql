CREATE TABLE IF NOT EXISTS entry_stats
(
    date_id        INTEGER PRIMARY KEY REFERENCES entries (date_id) ON DELETE CASCADE,
    word_count     INTEGER NOT NULL,
    unique_words   INTEGER NOT NULL,
    char_count     INTEGER NOT NULL,
    sentences      INTEGER NOT NULL,
    link_count     INTEGER NOT NULL,
    resource_count INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entry_words
(
    date_id INTEGER NOT NULL REFERENCES entries (date_id) ON DELETE CASCADE,
    word    TEXT    NOT NULL,
    n       INTEGER NOT NULL,
    PRIMARY KEY (date_id, word)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_entry_words_word ON entry_words (word);

CREATE TABLE IF NOT EXISTS words
(
    word    TEXT PRIMARY KEY,
    total   INTEGER NOT NULL,
    entries INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_words_total ON words (total DESC);
CREATE INDEX IF NOT EXISTS idx_words_entries ON words (entries DESC);

CREATE TRIGGER IF NOT EXISTS entry_words_ai
    AFTER INSERT
    ON entry_words
BEGIN
    INSERT INTO words (word, total, entries)
    VALUES (new.word, new.n, 1)
    ON CONFLICT(word) DO UPDATE SET total = total + new.n, entries = entries + 1;
END;

CREATE TRIGGER IF NOT EXISTS entry_words_ad
    AFTER DELETE
    ON entry_words
BEGIN
    UPDATE words SET total = total - old.n, entries = entries - 1 WHERE word = old.word;
    DELETE FROM words WHERE word = old.word AND entries <= 0;
END;

CREATE TRIGGER IF NOT EXISTS entry_words_au
    AFTER UPDATE
    ON entry_words
BEGIN
    UPDATE words SET total = total - old.n, entries = entries - 1 WHERE word = old.word;
    DELETE FROM words WHERE word = old.word AND entries <= 0;
    INSERT INTO words (word, total, entries)
    VALUES (new.word, new.n, 1)
    ON CONFLICT(word) DO UPDATE SET total = total + new.n, entries = entries + 1;
END;

CREATE TABLE IF NOT EXISTS resources
(
    id       INTEGER PRIMARY KEY,
    key      TEXT    NOT NULL UNIQUE,
    scheme   TEXT    NOT NULL,
    host     TEXT    NOT NULL,
    label    TEXT,
    refs     INTEGER NOT NULL DEFAULT 0,
    entries  INTEGER NOT NULL DEFAULT 0,
    credited INTEGER NOT NULL DEFAULT 0,
    first_id INTEGER,
    last_id  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_resources_refs ON resources (refs DESC);
CREATE INDEX IF NOT EXISTS idx_resources_host ON resources (host);

CREATE TABLE IF NOT EXISTS entry_resources
(
    date_id     INTEGER NOT NULL REFERENCES entries (date_id) ON DELETE CASCADE,
    resource_id INTEGER NOT NULL REFERENCES resources (id) ON DELETE CASCADE,
    n           INTEGER NOT NULL,
    anchor      TEXT    NOT NULL,
    in_credit   INTEGER NOT NULL,
    PRIMARY KEY (date_id, resource_id)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_entry_resources_resource ON entry_resources (resource_id);

CREATE TRIGGER IF NOT EXISTS entry_resources_ai
    AFTER INSERT
    ON entry_resources
BEGIN
    UPDATE resources
    SET refs     = refs + new.n,
        entries  = entries + 1,
        credited = credited + new.in_credit,
        first_id = min(coalesce(first_id, new.date_id), new.date_id),
        last_id  = max(coalesce(last_id, new.date_id), new.date_id),
        label    = (SELECT anchor
                    FROM entry_resources
                    WHERE resource_id = new.resource_id
                      AND anchor <> ''
                    GROUP BY anchor
                    ORDER BY COUNT(*) DESC, length(anchor) ASC
                    LIMIT 1)
    WHERE id = new.resource_id;
END;

CREATE TRIGGER IF NOT EXISTS entry_resources_ad
    AFTER DELETE
    ON entry_resources
BEGIN
    UPDATE resources
    SET refs     = refs - old.n,
        entries  = entries - 1,
        credited = credited - old.in_credit,
        first_id = (SELECT min(date_id) FROM entry_resources WHERE resource_id = old.resource_id),
        last_id  = (SELECT max(date_id) FROM entry_resources WHERE resource_id = old.resource_id),
        label    = (SELECT anchor
                    FROM entry_resources
                    WHERE resource_id = old.resource_id
                      AND anchor <> ''
                    GROUP BY anchor
                    ORDER BY COUNT(*) DESC, length(anchor) ASC
                    LIMIT 1)
    WHERE id = old.resource_id;
    DELETE FROM resources WHERE id = old.resource_id AND entries <= 0;
END;
