CREATE TABLE IF NOT EXISTS guilds
(
    guild_id     INTEGER PRIMARY KEY,
    channel_id   INTEGER,
    message      TEXT,
    explanation  TEXT    NOT NULL DEFAULT 'full',
    enabled      INTEGER NOT NULL DEFAULT 0,
    last_date_id INTEGER,
    updated_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_guilds_announcing ON guilds (enabled, last_date_id);
