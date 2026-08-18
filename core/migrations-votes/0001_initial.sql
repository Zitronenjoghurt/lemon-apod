CREATE TABLE IF NOT EXISTS voters
(
    id         BLOB PRIMARY KEY,             -- 16 random bytes, minted server side on a first vote
    created_at INTEGER NOT NULL,
    last_seen  INTEGER NOT NULL,
    cohort     BLOB,                         -- coarse HMAC, abuse control only, expires
    cohort_at  INTEGER,
    weight     REAL    NOT NULL DEFAULT 1.0, -- Crowd-BT reliability, written by the fit
    blocked    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_voters_cohort ON voters (cohort) WHERE cohort IS NOT NULL;

CREATE TABLE IF NOT EXISTS votes
(
    id        INTEGER PRIMARY KEY,
    voter_id  BLOB    NOT NULL REFERENCES voters (id) ON DELETE CASCADE,
    category  TEXT    NOT NULL,
    left_id   INTEGER NOT NULL,          -- date_id of the entry rendered on the left
    right_id  INTEGER NOT NULL,
    outcome   TEXT    NOT NULL CHECK (outcome IN ('left', 'right', 'tie')),
    issued_at INTEGER NOT NULL,
    voted_at  INTEGER NOT NULL,
    probe     INTEGER NOT NULL DEFAULT 0 -- a pair this voter has judged before, sides swapped
);

CREATE INDEX IF NOT EXISTS idx_votes_voter ON votes (voter_id, voted_at);
CREATE INDEX IF NOT EXISTS idx_votes_pair ON votes (category, left_id, right_id);

-- A cache of the fit, not the source of truth
CREATE TABLE IF NOT EXISTS scores
(
    category    TEXT    NOT NULL,
    picture_id  INTEGER NOT NULL, -- picture_group, the date the picture was first published
    score       REAL    NOT NULL,
    stderr      REAL    NOT NULL,
    comparisons INTEGER NOT NULL,
    prior_score REAL,             -- from the committed baseline, if any
    prior_ess   REAL,
    PRIMARY KEY (category, picture_id)
);

-- So a score can always say what produced it.
CREATE TABLE IF NOT EXISTS fits
(
    id         INTEGER PRIMARY KEY,
    category   TEXT    NOT NULL,
    ran_at     INTEGER NOT NULL,
    model      TEXT    NOT NULL, -- version string
    votes      INTEGER NOT NULL,
    iterations INTEGER NOT NULL,
    side_bias  REAL
);

CREATE INDEX IF NOT EXISTS idx_fits_category ON fits (category, ran_at);
