use super::baseline::Dataset;
use super::fit::{Anchor, Fit, Outcome, Score, Vote};
use super::model::Category;
use super::{COMPARISON_INFORMATION, MODEL, Z};
use crate::date::ApodDate;
use crate::db::{Db, DbConfig, DbResult};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::Row;
use sqlx::migrate::Migrator;
use std::fmt;
use std::path::Path;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations-votes");

pub const VOTER_ID_BYTES: usize = 16;
const MAX_AVOID: i64 = 500;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoterId([u8; VOTER_ID_BYTES]);

impl VoterId {
    pub const fn new(bytes: [u8; VOTER_ID_BYTES]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        if text.len() != VOTER_ID_BYTES * 2 {
            return None;
        }

        let mut bytes = [0u8; VOTER_ID_BYTES];
        for (at, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(text.get(at * 2..at * 2 + 2)?, 16).ok()?;
        }

        Some(Self(bytes))
    }

    fn from_blob(blob: &[u8]) -> Option<Self> {
        blob.try_into().ok().map(Self)
    }
}

impl fmt::Debug for VoterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "voter:{:02x}{:02x}…", self.0[0], self.0[1])
    }
}

#[derive(Debug, Clone)]
pub struct Voter {
    pub id: VoterId,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub cohort: Option<Vec<u8>>,
    pub weight: f64,
    pub blocked: bool,
}

#[derive(Debug, Clone)]
pub struct Cast {
    pub voter: VoterId,
    pub cohort: Option<Vec<u8>>,
    pub category: Category,
    pub left: ApodDate,
    pub right: ApodDate,
    pub outcome: Outcome,
    pub issued_at: DateTime<Utc>,
    pub voted_at: DateTime<Utc>,
    pub probe: bool,
}

impl Cast {
    pub fn response(&self) -> chrono::TimeDelta {
        self.voted_at - self.issued_at
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Whose<'a> {
    Voter(VoterId),
    Cohort(&'a [u8]),
}

#[derive(Debug, Clone, Default)]
pub struct Standing {
    pub votes: u64,
    pub avoid: Vec<ApodDate>,
    pub probe: Option<(ApodDate, ApodDate)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranked {
    pub score: Score,
    pub inherited: f64,
}

impl Ranked {
    pub fn evidence(&self) -> f64 {
        f64::from(self.score.comparisons) + self.inherited
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Consistency {
    pub voter: VoterId,
    pub probes: u64,
    pub agreed: u64,
    pub expected: f64,
    pub quick: u64,
    pub votes: u64,
    pub weight: f64,
    pub blocked: bool,
}

const CHANCE: f64 = 0.5;

type ProbeRow = (
    Vec<u8>,
    String,
    Option<String>,
    Option<f64>,
    Option<f64>,
    f64,
    i64,
);

impl Consistency {
    pub fn observed(&self) -> f64 {
        match self.probes {
            0 => 1.0,
            probes => self.agreed as f64 / probes as f64,
        }
    }

    pub fn reliability(&self) -> f64 {
        let headroom = self.expected - CHANCE;
        if self.probes < super::MIN_PROBES || headroom <= 1e-6 {
            return 1.0;
        }

        ((self.observed() - CHANCE) / headroom).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tally {
    pub votes: u64,
    pub voters: u64,
    pub ran_at: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub side_bias: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct VoteStore {
    db: Db,
}

impl VoteStore {
    pub async fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let db = Db::open(DbConfig::read_write(path.as_ref())).await?;
        db.migrate(&MIGRATIONS).await?;
        Ok(Self { db })
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub async fn close(&self) {
        self.db.close().await;
    }

    pub async fn voter(&self, id: VoterId) -> DbResult<Option<Voter>> {
        let row = sqlx::query(
            "SELECT id, created_at, last_seen, cohort, weight, blocked FROM voters WHERE id = ?1",
        )
        .bind(id.bytes().to_vec())
        .fetch_optional(self.db.reader())
        .await?;

        Ok(row.as_ref().and_then(read_voter))
    }

    pub async fn record(&self, cast: &Cast, cohort_expires_at: DateTime<Utc>) -> DbResult<i64> {
        let mut tx = self.db.writer()?.begin().await?;
        let now = millis(cast.voted_at);

        sqlx::query(
            "INSERT INTO voters (id, created_at, last_seen, cohort, cohort_at)
             VALUES (?1, ?2, ?2, ?3, ?2)
             ON CONFLICT(id) DO UPDATE SET
               last_seen = ?2,
               cohort    = CASE WHEN cohort_at IS NULL OR cohort_at < ?4
                                THEN ?3 ELSE cohort END,
               cohort_at = CASE WHEN cohort_at IS NULL OR cohort_at < ?4
                                THEN ?2 ELSE cohort_at END",
        )
        .bind(cast.voter.bytes().to_vec())
        .bind(now)
        .bind(cast.cohort.clone())
        .bind(millis(cohort_expires_at))
        .execute(&mut *tx)
        .await?;

        let done = sqlx::query(
            "INSERT INTO votes
               (voter_id, category, left_id, right_id, outcome, issued_at, voted_at, probe)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(cast.voter.bytes().to_vec())
        .bind(cast.category.as_str())
        .bind(cast.left.days())
        .bind(cast.right.days())
        .bind(cast.outcome.as_str())
        .bind(millis(cast.issued_at))
        .bind(now)
        .bind(i64::from(cast.probe))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(done.last_insert_rowid())
    }

    pub async fn standing(
        &self,
        voter: VoterId,
        category: Category,
        recent: usize,
        per_picture: u32,
        since: DateTime<Utc>,
        probe_before: DateTime<Utc>,
    ) -> DbResult<Standing> {
        let id = voter.bytes().to_vec();

        let votes: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM votes WHERE voter_id = ?1 AND voted_at >= ?2")
                .bind(&id)
                .bind(millis(since))
                .fetch_one(self.db.reader())
                .await?;

        let seen: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT left_id, right_id FROM votes
             WHERE voter_id = ?1 AND category = ?2
             ORDER BY id DESC LIMIT ?3",
        )
        .bind(&id)
        .bind(category.as_str())
        .bind(recent as i64)
        .fetch_all(self.db.reader())
        .await?;

        let capped: Vec<i64> = sqlx::query_scalar(
            "SELECT entry FROM (
               SELECT left_id AS entry FROM votes WHERE voter_id = ?1 AND category = ?2
               UNION ALL
               SELECT right_id AS entry FROM votes WHERE voter_id = ?1 AND category = ?2
             )
             GROUP BY entry HAVING COUNT(*) >= ?3
             ORDER BY COUNT(*) DESC, entry
             LIMIT ?4",
        )
        .bind(&id)
        .bind(category.as_str())
        .bind(i64::from(per_picture))
        .bind(MAX_AVOID)
        .fetch_all(self.db.reader())
        .await?;

        let probe: Option<(i64, i64)> = sqlx::query_as(
            "SELECT left_id, right_id FROM votes
             WHERE voter_id = ?1 AND category = ?2 AND probe = 0 AND voted_at <= ?3
             ORDER BY RANDOM() LIMIT 1",
        )
        .bind(&id)
        .bind(category.as_str())
        .bind(millis(probe_before))
        .fetch_optional(self.db.reader())
        .await?;

        let mut avoid: Vec<ApodDate> = seen
            .into_iter()
            .flat_map(|(left, right)| [left, right])
            .chain(capped)
            .map(days)
            .collect();
        avoid.sort_unstable();
        avoid.dedup();

        Ok(Standing {
            votes: votes.max(0) as u64,
            avoid,
            probe: probe.map(|(left, right)| (days(left), days(right))),
        })
    }

    pub async fn cohort_votes(&self, cohort: &[u8], since: DateTime<Utc>) -> DbResult<u64> {
        let votes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM votes
             JOIN voters ON voters.id = votes.voter_id
             WHERE voters.cohort = ?1 AND votes.voted_at >= ?2",
        )
        .bind(cohort.to_vec())
        .bind(millis(since))
        .fetch_one(self.db.reader())
        .await?;

        Ok(votes.max(0) as u64)
    }

    pub async fn ages_out_at(
        &self,
        whose: Whose<'_>,
        since: DateTime<Utc>,
        over_by: u64,
    ) -> DbResult<Option<DateTime<Utc>>> {
        let query = match whose {
            Whose::Voter(voter) => sqlx::query_scalar(
                "SELECT voted_at FROM votes
                 WHERE voter_id = ?1 AND voted_at >= ?2
                 ORDER BY voted_at LIMIT 1 OFFSET ?3",
            )
            .bind(voter.bytes().to_vec()),
            Whose::Cohort(cohort) => sqlx::query_scalar(
                "SELECT votes.voted_at FROM votes
                 JOIN voters ON voters.id = votes.voter_id
                 WHERE voters.cohort = ?1 AND votes.voted_at >= ?2
                 ORDER BY votes.voted_at LIMIT 1 OFFSET ?3",
            )
            .bind(cohort.to_vec()),
        };

        let voted_at: Option<i64> = query
            .bind(millis(since))
            .bind(over_by as i64)
            .fetch_optional(self.db.reader())
            .await?;

        Ok(voted_at.map(at))
    }

    pub async fn consistency(&self, quick: chrono::TimeDelta) -> DbResult<Vec<Consistency>> {
        let pace: Vec<(Vec<u8>, i64, i64)> = sqlx::query_as(
            "SELECT voter_id,
                    COUNT(*),
                    SUM(CASE WHEN voted_at - issued_at < ?1 THEN 1 ELSE 0 END)
             FROM votes GROUP BY voter_id",
        )
        .bind(quick.num_milliseconds())
        .fetch_all(self.db.reader())
        .await?;

        let pace: std::collections::HashMap<VoterId, (u64, u64)> = pace
            .into_iter()
            .filter_map(|(id, votes, quick)| {
                Some((
                    VoterId::from_blob(&id)?,
                    (votes.max(0) as u64, quick.max(0) as u64),
                ))
            })
            .collect();

        let rows: Vec<ProbeRow> = sqlx::query_as(
            "SELECT p.voter_id,
                        p.outcome,
                        (SELECT f.outcome FROM votes f
                          WHERE f.voter_id = p.voter_id AND f.category = p.category
                            AND f.probe = 0
                            AND f.left_id = p.right_id AND f.right_id = p.left_id
                            AND f.id < p.id
                          ORDER BY f.id DESC LIMIT 1),
                        (SELECT sl.score FROM scores sl
                          WHERE sl.category = p.category AND sl.picture_id = p.left_id),
                        (SELECT sr.score FROM scores sr
                          WHERE sr.category = p.category AND sr.picture_id = p.right_id),
                        voters.weight,
                        voters.blocked
                 FROM votes p
                 JOIN voters ON voters.id = p.voter_id
                 WHERE p.probe = 1
                 ORDER BY p.voter_id",
        )
        .fetch_all(self.db.reader())
        .await?;

        let mut found: Vec<Consistency> = Vec::new();
        let mut headroom: Vec<f64> = Vec::new();

        for (id, said_now, said_before, left, right, weight, blocked) in rows {
            let (Some(voter), Some(before)) = (VoterId::from_blob(&id), said_before) else {
                continue;
            };
            let (Ok(now), Ok(before)) = (said_now.parse::<Outcome>(), before.parse::<Outcome>())
            else {
                continue;
            };

            if found.last().is_none_or(|last| last.voter != voter) {
                let &(votes, quick) = pace.get(&voter).unwrap_or(&(0, 0));
                found.push(Consistency {
                    voter,
                    probes: 0,
                    agreed: 0,
                    expected: 0.0,
                    quick,
                    votes,
                    weight,
                    blocked: blocked != 0,
                });
                headroom.push(0.0);
            }

            let held = found.last_mut().expect("just pushed");
            held.probes += 1;
            if now == mirror(before) {
                held.agreed += 1;
            }

            let expected = match (left, right) {
                (Some(left), Some(right)) => {
                    let odds = logistic(left - right);
                    odds * odds + (1.0 - odds) * (1.0 - odds)
                }
                _ => CHANCE,
            };
            *headroom.last_mut().expect("just pushed") += expected;
        }

        for (held, total) in found.iter_mut().zip(headroom) {
            held.expected = (total / held.probes as f64).max(CHANCE);
        }

        found.sort_by(|one, two| one.reliability().total_cmp(&two.reliability()));
        Ok(found)
    }

    pub async fn kin(&self, cohort: &[u8]) -> DbResult<Vec<VoterId>> {
        let blobs: Vec<Vec<u8>> =
            sqlx::query_scalar("SELECT id FROM voters WHERE cohort = ?1 ORDER BY created_at")
                .bind(cohort.to_vec())
                .fetch_all(self.db.reader())
                .await?;

        Ok(blobs
            .iter()
            .filter_map(|blob| VoterId::from_blob(blob))
            .collect())
    }

    pub async fn log(&self, category: Category) -> DbResult<Vec<Vote>> {
        let rows: Vec<(i64, i64, String, f64)> = sqlx::query_as(
            "SELECT votes.left_id, votes.right_id, votes.outcome, voters.weight
             FROM votes JOIN voters ON voters.id = votes.voter_id
             WHERE votes.category = ?1 AND voters.blocked = 0 AND votes.probe = 0
             ORDER BY votes.id",
        )
        .bind(category.as_str())
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(left, right, outcome, weight)| {
                Some(Vote {
                    left: days(left),
                    right: days(right),
                    outcome: outcome.parse().ok()?,
                    weight,
                })
            })
            .collect())
    }

    pub async fn anchors(&self, category: Category) -> DbResult<Vec<Anchor>> {
        let rows: Vec<(i64, f64, f64)> = sqlx::query_as(
            "SELECT picture_id, prior_score, prior_ess FROM scores
             WHERE category = ?1 AND prior_score IS NOT NULL AND prior_ess IS NOT NULL
             ORDER BY picture_id",
        )
        .bind(category.as_str())
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(picture, score, ess)| Anchor {
                picture: days(picture),
                score,
                ess,
            })
            .collect())
    }

    pub async fn save(&self, category: Category, fit: &Fit) -> DbResult<()> {
        let mut tx = self.db.writer()?.begin().await?;

        sqlx::query("DELETE FROM scores WHERE category = ?1 AND prior_score IS NULL")
            .bind(category.as_str())
            .execute(&mut *tx)
            .await?;

        for score in &fit.scores {
            sqlx::query(
                "INSERT INTO scores (category, picture_id, score, stderr, comparisons)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(category, picture_id) DO UPDATE SET
                   score = excluded.score,
                   stderr = excluded.stderr,
                   comparisons = excluded.comparisons",
            )
            .bind(category.as_str())
            .bind(score.picture.days())
            .bind(score.score)
            .bind(score.stderr)
            .bind(i64::from(score.comparisons))
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "INSERT INTO fits (category, ran_at, model, votes, iterations, side_bias)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(category.as_str())
        .bind(millis(Utc::now()))
        .bind(MODEL)
        .bind(fit.votes as i64)
        .bind(i64::from(fit.iterations))
        .bind(fit.side_bias)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn import(&self, dataset: &Dataset) -> DbResult<usize> {
        let mut tx = self.db.writer()?.begin().await?;

        sqlx::query(
            "UPDATE scores SET prior_score = NULL, prior_ess = NULL
             WHERE category = ?1 AND prior_score IS NOT NULL",
        )
        .bind(dataset.category.as_str())
        .execute(&mut *tx)
        .await?;

        let mut loaded = 0;
        for anchor in dataset.anchors() {
            let stderr = (anchor.ess * COMPARISON_INFORMATION)
                .max(f64::MIN_POSITIVE)
                .sqrt()
                .recip();

            sqlx::query(
                "INSERT INTO scores
                   (category, picture_id, score, stderr, comparisons, prior_score, prior_ess)
                 VALUES (?1, ?2, ?3, ?4, 0, ?3, ?5)
                 ON CONFLICT(category, picture_id) DO UPDATE SET
                   prior_score = excluded.prior_score,
                   prior_ess = excluded.prior_ess",
            )
            .bind(dataset.category.as_str())
            .bind(anchor.picture.days())
            .bind(anchor.score)
            .bind(stderr)
            .bind(anchor.ess)
            .execute(&mut *tx)
            .await?;
            loaded += 1;
        }

        tx.commit().await?;
        Ok(loaded)
    }

    pub async fn scores(&self, category: Category) -> DbResult<Vec<Score>> {
        let rows: Vec<(i64, f64, f64, i64)> = sqlx::query_as(
            "SELECT picture_id, score, stderr, comparisons FROM scores
             WHERE category = ?1 ORDER BY picture_id",
        )
        .bind(category.as_str())
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows.into_iter().map(read_score).collect())
    }

    pub async fn board(
        &self,
        category: Category,
        min_comparisons: u32,
        limit: i64,
        offset: i64,
    ) -> DbResult<Vec<Ranked>> {
        let rows: Vec<(i64, f64, f64, i64, Option<f64>)> = sqlx::query_as(
            "SELECT picture_id, score, stderr, comparisons, prior_ess FROM scores
             WHERE category = ?1 AND comparisons + COALESCE(prior_ess, 0) >= ?2
             ORDER BY score - ?3 * stderr DESC, picture_id
             LIMIT ?4 OFFSET ?5",
        )
        .bind(category.as_str())
        .bind(f64::from(min_comparisons))
        .bind(Z)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(picture, score, stderr, comparisons, inherited)| Ranked {
                score: read_score((picture, score, stderr, comparisons)),
                inherited: inherited.unwrap_or(0.0),
            })
            .collect())
    }

    pub async fn board_size(&self, category: Category, min_comparisons: u32) -> DbResult<u64> {
        let rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM scores
             WHERE category = ?1 AND comparisons + COALESCE(prior_ess, 0) >= ?2",
        )
        .bind(category.as_str())
        .bind(f64::from(min_comparisons))
        .fetch_one(self.db.reader())
        .await?;

        Ok(rows.max(0) as u64)
    }

    pub async fn tally(&self, category: Category) -> DbResult<Tally> {
        let votes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM votes WHERE category = ?1")
            .bind(category.as_str())
            .fetch_one(self.db.reader())
            .await?;

        let voters: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voters WHERE blocked = 0")
            .fetch_one(self.db.reader())
            .await?;

        let last: Option<(i64, String, Option<f64>)> = sqlx::query_as(
            "SELECT ran_at, model, side_bias FROM fits
             WHERE category = ?1 ORDER BY ran_at DESC, id DESC LIMIT 1",
        )
        .bind(category.as_str())
        .fetch_optional(self.db.reader())
        .await?;

        Ok(Tally {
            votes: votes.max(0) as u64,
            voters: voters.max(0) as u64,
            ran_at: last.as_ref().map(|(ran_at, ..)| at(*ran_at)),
            model: last.as_ref().map(|(_, model, _)| model.clone()),
            side_bias: last.and_then(|(.., bias)| bias),
        })
    }

    pub async fn forget(&self, voter: VoterId) -> DbResult<u64> {
        let id = voter.bytes().to_vec();

        let votes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM votes WHERE voter_id = ?1")
            .bind(&id)
            .fetch_one(self.db.reader())
            .await?;

        sqlx::query("DELETE FROM voters WHERE id = ?1")
            .bind(&id)
            .execute(self.db.writer()?)
            .await?;

        Ok(votes.max(0) as u64)
    }

    pub async fn block(&self, voter: VoterId, blocked: bool) -> DbResult<bool> {
        let done = sqlx::query("UPDATE voters SET blocked = ?2 WHERE id = ?1")
            .bind(voter.bytes().to_vec())
            .bind(i64::from(blocked))
            .execute(self.db.writer()?)
            .await?;

        Ok(done.rows_affected() == 1)
    }

    pub async fn weigh(&self, voter: VoterId, weight: f64) -> DbResult<bool> {
        let done = sqlx::query("UPDATE voters SET weight = ?2 WHERE id = ?1")
            .bind(voter.bytes().to_vec())
            .bind(weight)
            .execute(self.db.writer()?)
            .await?;

        Ok(done.rows_affected() == 1)
    }

    pub async fn expire_cohorts(&self, before: DateTime<Utc>) -> DbResult<u64> {
        let done = sqlx::query(
            "UPDATE voters SET cohort = NULL, cohort_at = NULL
             WHERE cohort IS NOT NULL AND cohort_at < ?1",
        )
        .bind(millis(before))
        .execute(self.db.writer()?)
        .await?;

        Ok(done.rows_affected())
    }

    pub async fn forget_stale(&self, before: DateTime<Utc>) -> DbResult<u64> {
        let done = sqlx::query("DELETE FROM voters WHERE last_seen < ?1")
            .bind(millis(before))
            .execute(self.db.writer()?)
            .await?;

        Ok(done.rows_affected())
    }
}

fn read_voter(row: &sqlx::sqlite::SqliteRow) -> Option<Voter> {
    Some(Voter {
        id: VoterId::from_blob(&row.get::<Vec<u8>, _>("id"))?,
        created_at: at(row.get("created_at")),
        last_seen: at(row.get("last_seen")),
        cohort: row.get("cohort"),
        weight: row.get("weight"),
        blocked: row.get::<i64, _>("blocked") != 0,
    })
}

fn read_score((picture, score, stderr, comparisons): (i64, f64, f64, i64)) -> Score {
    Score {
        picture: days(picture),
        score,
        stderr,
        comparisons: comparisons.max(0) as u32,
    }
}

fn mirror(outcome: Outcome) -> Outcome {
    match outcome {
        Outcome::Left => Outcome::Right,
        Outcome::Right => Outcome::Left,
        Outcome::Tie => Outcome::Tie,
    }
}

fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn days(value: i64) -> ApodDate {
    ApodDate::from_days(value as i32)
}

fn millis(at: DateTime<Utc>) -> i64 {
    at.timestamp_millis()
}

fn at(millis: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(millis)
        .single()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rating::baseline::Row;
    use crate::rating::{Grouping, MIN_COMPARISONS, Prior, fit};
    use chrono::TimeDelta;

    async fn store() -> VoteStore {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let dir = std::env::temp_dir().join(format!(
            "apod-votes-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        VoteStore::open(dir.join("votes.db")).await.unwrap()
    }

    fn voter(seed: u8) -> VoterId {
        VoterId::new([seed; VOTER_ID_BYTES])
    }

    fn day(n: i32) -> ApodDate {
        ApodDate::from_days(n)
    }

    fn cast(who: VoterId, left: i32, right: i32, outcome: Outcome) -> Cast {
        let now = Utc::now();
        Cast {
            voter: who,
            cohort: Some(b"cohort".to_vec()),
            category: Category::Beautiful,
            left: day(left),
            right: day(right),
            outcome,
            issued_at: now - TimeDelta::seconds(3),
            voted_at: now,
            probe: false,
        }
    }

    fn never() -> DateTime<Utc> {
        Utc::now() - TimeDelta::days(365)
    }

    fn quick() -> TimeDelta {
        TimeDelta::milliseconds(super::super::QUICK_RESPONSE_MS)
    }

    #[tokio::test]
    async fn the_first_vote_mints_the_voter_and_later_ones_only_touch_the_clock() {
        let store = store().await;
        let who = voter(1);

        assert!(store.voter(who).await.unwrap().is_none(), "nothing yet");

        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        let minted = store.voter(who).await.unwrap().unwrap();
        assert_eq!(minted.weight, 1.0);
        assert!(!minted.blocked);
        assert_eq!(minted.cohort.as_deref(), Some(&b"cohort"[..]));

        store
            .record(&cast(who, 2, 3, Outcome::Right), never())
            .await
            .unwrap();
        let again = store.voter(who).await.unwrap().unwrap();
        assert_eq!(again.created_at, minted.created_at, "minted once");
        assert!(again.last_seen >= minted.last_seen);
        assert_eq!(store.tally(Category::Beautiful).await.unwrap().voters, 1);
    }

    #[tokio::test]
    async fn a_cohort_is_kept_until_it_ages_out_rather_than_refreshed_every_vote() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();

        let mut later = cast(who, 2, 3, Outcome::Left);
        later.cohort = Some(b"moved-house".to_vec());
        store.record(&later, never()).await.unwrap();
        assert_eq!(
            store.voter(who).await.unwrap().unwrap().cohort.as_deref(),
            Some(&b"cohort"[..]),
            "a fresh cohort every vote would be a sliding window, not a fixed one"
        );

        store
            .record(&later, Utc::now() + TimeDelta::days(1))
            .await
            .unwrap();
        assert_eq!(
            store.voter(who).await.unwrap().unwrap().cohort.as_deref(),
            Some(&b"moved-house"[..])
        );
    }

    async fn separated(store: &VoteStore, pairs: i32) {
        let mut log = Vec::new();
        for at in 0..pairs {
            let (left, right) = (day(at * 2), day(at * 2 + 1));
            for round in 0..20 {
                let outcome = match round < 18 {
                    true => Outcome::Left,
                    false => Outcome::Right,
                };
                log.push(Vote::new(left, right, outcome));
            }
        }

        let decided = fit(&log, &Grouping::default(), &Prior::weak());
        store.save(Category::Beautiful, &decided).await.unwrap();
    }

    fn probe(who: VoterId, left: i32, right: i32, outcome: Outcome) -> Cast {
        Cast {
            probe: true,
            ..cast(who, left, right, outcome)
        }
    }

    #[tokio::test]
    async fn a_probe_is_measurement_rather_than_evidence_and_stays_out_of_the_fit() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        store
            .record(&probe(who, 1, 0, Outcome::Right), never())
            .await
            .unwrap();

        let log = store.log(Category::Beautiful).await.unwrap();
        assert_eq!(
            log.len(),
            1,
            "one person's opinion of one pair is one comparison, however often it is asked"
        );
    }

    #[tokio::test]
    async fn a_voter_who_says_the_same_thing_both_ways_round_reads_as_consistent() {
        let store = store().await;
        let who = voter(1);
        separated(&store, 4).await;

        for at in 0..4 {
            let (left, right) = (at * 2, at * 2 + 1);
            store
                .record(&cast(who, left, right, Outcome::Left), never())
                .await
                .unwrap();
            // The pair comes back mirrored, so naming the same picture means answering right.
            store
                .record(&probe(who, right, left, Outcome::Right), never())
                .await
                .unwrap();
        }

        let found = store.consistency(quick()).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].probes, found[0].agreed), (4, 4));
        assert_eq!(found[0].observed(), 1.0);
        assert_eq!(found[0].reliability(), 1.0);
    }

    #[tokio::test]
    async fn a_voter_who_contradicts_themselves_every_time_reads_as_a_coin() {
        let store = store().await;
        let who = voter(1);
        separated(&store, 4).await;

        for at in 0..4 {
            let (left, right) = (at * 2, at * 2 + 1);
            store
                .record(&cast(who, left, right, Outcome::Left), never())
                .await
                .unwrap();
            store
                .record(&probe(who, right, left, Outcome::Left), never())
                .await
                .unwrap();
        }

        let found = store.consistency(quick()).await.unwrap();
        assert_eq!((found[0].probes, found[0].agreed), (4, 0));
        assert_eq!(found[0].reliability(), 0.0);
    }

    #[tokio::test]
    async fn a_close_pair_is_not_held_against_the_voter_who_flipped_on_it() {
        let store = store().await;
        let who = voter(1);

        let mut log = Vec::new();
        for at in 0..4 {
            let (left, right) = (day(at * 2), day(at * 2 + 1));
            for round in 0..20 {
                let outcome = match round % 2 == 0 {
                    true => Outcome::Left,
                    false => Outcome::Right,
                };
                log.push(Vote::new(left, right, outcome));
            }
        }
        let level = fit(&log, &Grouping::default(), &Prior::weak());
        store.save(Category::Beautiful, &level).await.unwrap();

        for at in 0..4 {
            let (left, right) = (at * 2, at * 2 + 1);
            store
                .record(&cast(who, left, right, Outcome::Left), never())
                .await
                .unwrap();
            store
                .record(&probe(who, right, left, Outcome::Left), never())
                .await
                .unwrap();
        }

        let found = store.consistency(quick()).await.unwrap();
        assert_eq!(found[0].probes, 4);
        assert_eq!(found[0].agreed, 0, "they did contradict themselves");
        assert!(
            found[0].expected < 0.55,
            "pictures this level are a coin toss, and the expectation has to say so, got {}",
            found[0].expected
        );
        assert_eq!(
            found[0].reliability(),
            1.0,
            "flipping on pairs nobody can separate is not evidence of anything"
        );
    }

    #[tokio::test]
    async fn a_voter_barely_probed_yet_is_given_the_benefit_of_the_doubt() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        store
            .record(&probe(who, 1, 0, Outcome::Left), never())
            .await
            .unwrap();

        let found = store.consistency(quick()).await.unwrap();
        assert_eq!((found[0].probes, found[0].agreed), (1, 0));
        assert_eq!(
            found[0].reliability(),
            1.0,
            "one contradiction is a bad day, not a pattern"
        );
    }

    #[tokio::test]
    async fn a_probe_with_nothing_behind_it_is_not_counted() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&probe(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        assert!(
            store.consistency(quick()).await.unwrap().is_empty(),
            "a probe whose original vote was erased has nothing to compare against"
        );
    }

    #[tokio::test]
    async fn the_least_consistent_voter_is_the_one_reported_first() {
        let store = store().await;
        let (steady, erratic) = (voter(1), voter(2));
        separated(&store, 4).await;

        for (who, agreeing) in [(steady, true), (erratic, false)] {
            for at in 0..4 {
                let (left, right) = (at * 2, at * 2 + 1);
                store
                    .record(&cast(who, left, right, Outcome::Left), never())
                    .await
                    .unwrap();
                let said = match agreeing {
                    true => Outcome::Right,
                    false => Outcome::Left,
                };
                store
                    .record(&probe(who, right, left, said), never())
                    .await
                    .unwrap();
            }
        }

        let found = store.consistency(quick()).await.unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].voter, erratic);
        assert_eq!(found[1].voter, steady);
    }

    #[tokio::test]
    async fn the_log_comes_back_in_the_order_it_was_written_and_carries_the_weights() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        store
            .record(&cast(who, 1, 2, Outcome::Tie), never())
            .await
            .unwrap();
        store.weigh(who, 0.25).await.unwrap();

        let log = store.log(Category::Beautiful).await.unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].outcome, Outcome::Left);
        assert_eq!(log[1].outcome, Outcome::Tie);
        assert!(log.iter().all(|vote| vote.weight == 0.25));

        assert!(
            store.log(Category::Fascinating).await.unwrap().is_empty(),
            "the categories are separate models"
        );
    }

    #[tokio::test]
    async fn blocking_takes_a_voter_out_of_the_fit_and_leaves_the_row_alone() {
        let store = store().await;
        let (honest, troll) = (voter(1), voter(2));

        store
            .record(&cast(honest, 0, 1, Outcome::Left), never())
            .await
            .unwrap();
        for _ in 0..5 {
            store
                .record(&cast(troll, 0, 1, Outcome::Right), never())
                .await
                .unwrap();
        }

        assert_eq!(store.log(Category::Beautiful).await.unwrap().len(), 6);
        assert!(store.block(troll, true).await.unwrap());
        assert_eq!(store.log(Category::Beautiful).await.unwrap().len(), 1);
        assert_eq!(
            store.tally(Category::Beautiful).await.unwrap().votes,
            6,
            "the votes are still on record"
        );

        store.block(troll, false).await.unwrap();
        assert_eq!(store.log(Category::Beautiful).await.unwrap().len(), 6);
    }

    #[tokio::test]
    async fn forgetting_a_voter_takes_their_votes_with_them() {
        let store = store().await;
        let (one, two) = (voter(1), voter(2));

        for _ in 0..3 {
            store
                .record(&cast(one, 0, 1, Outcome::Left), never())
                .await
                .unwrap();
        }
        store
            .record(&cast(two, 0, 1, Outcome::Right), never())
            .await
            .unwrap();

        assert_eq!(store.forget(one).await.unwrap(), 3);
        assert!(store.voter(one).await.unwrap().is_none());
        assert_eq!(
            store.log(Category::Beautiful).await.unwrap().len(),
            1,
            "the cascade has to be enforced, not hoped for"
        );
        assert_eq!(store.forget(voter(9)).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_voter_is_not_shown_a_picture_they_just_judged() {
        let store = store().await;
        let who = voter(1);

        store
            .record(&cast(who, 10, 20, Outcome::Left), never())
            .await
            .unwrap();
        store
            .record(&cast(who, 30, 40, Outcome::Right), never())
            .await
            .unwrap();

        let standing = store
            .standing(who, Category::Beautiful, 1, 99, never(), Utc::now())
            .await
            .unwrap();

        assert_eq!(
            standing.avoid,
            vec![day(30), day(40)],
            "only the last ballot"
        );
        assert_eq!(standing.votes, 2);
    }

    #[tokio::test]
    async fn a_picture_a_voter_has_already_had_their_say_on_stops_coming_up() {
        let store = store().await;
        let who = voter(1);

        for other in 20..23 {
            store
                .record(&cast(who, 10, other, Outcome::Left), never())
                .await
                .unwrap();
        }

        let standing = store
            .standing(who, Category::Beautiful, 0, 3, never(), Utc::now())
            .await
            .unwrap();

        assert_eq!(
            standing.avoid,
            vec![day(10)],
            "no individual gets to move one score on their own"
        );
    }

    #[tokio::test]
    async fn a_repeat_needs_a_pair_the_voter_judged_a_while_ago() {
        let store = store().await;
        let who = voter(1);

        let mut old = cast(who, 10, 20, Outcome::Left);
        old.voted_at = Utc::now() - TimeDelta::minutes(10);
        old.issued_at = old.voted_at - TimeDelta::seconds(4);
        store.record(&old, never()).await.unwrap();
        store
            .record(&cast(who, 30, 40, Outcome::Left), never())
            .await
            .unwrap();

        let ready = store
            .standing(
                who,
                Category::Beautiful,
                0,
                99,
                never(),
                Utc::now() - TimeDelta::minutes(5),
            )
            .await
            .unwrap();
        assert_eq!(
            ready.probe,
            Some((day(10), day(20))),
            "the pair from ten minutes ago, not the one from a second ago"
        );

        let nothing_yet = store
            .standing(
                who,
                Category::Beautiful,
                0,
                99,
                never(),
                Utc::now() - TimeDelta::hours(1),
            )
            .await
            .unwrap();
        assert_eq!(nothing_yet.probe, None);
    }

    #[tokio::test]
    async fn a_probe_is_not_offered_as_a_probe_again() {
        let store = store().await;
        let who = voter(1);

        let mut probed = cast(who, 10, 20, Outcome::Left);
        probed.probe = true;
        probed.voted_at = Utc::now() - TimeDelta::minutes(10);
        store.record(&probed, never()).await.unwrap();

        let standing = store
            .standing(
                who,
                Category::Beautiful,
                0,
                99,
                never(),
                Utc::now() - TimeDelta::minutes(5),
            )
            .await
            .unwrap();
        assert_eq!(standing.probe, None);
    }

    #[tokio::test]
    async fn a_budget_counts_only_the_window_and_counts_every_category_in_it() {
        let store = store().await;
        let who = voter(1);

        let mut old = cast(who, 0, 1, Outcome::Left);
        old.voted_at = Utc::now() - TimeDelta::hours(3);
        store.record(&old, never()).await.unwrap();

        let mut other = cast(who, 2, 3, Outcome::Left);
        other.category = Category::Fascinating;
        store.record(&other, never()).await.unwrap();
        store
            .record(&cast(who, 4, 5, Outcome::Left), never())
            .await
            .unwrap();

        let hour = store
            .standing(
                who,
                Category::Beautiful,
                0,
                99,
                Utc::now() - TimeDelta::hours(1),
                Utc::now(),
            )
            .await
            .unwrap();
        assert_eq!(hour.votes, 2, "the three-hour-old one has aged out");
    }

    #[tokio::test]
    async fn a_window_frees_up_when_enough_of_it_has_aged_out_and_not_before() {
        let store = store().await;
        let who = voter(1);

        for minutes in [50i64, 40, 30] {
            let mut vote = cast(who, minutes as i32, minutes as i32 + 1, Outcome::Left);
            vote.voted_at = Utc::now() - TimeDelta::minutes(minutes);
            store.record(&vote, never()).await.unwrap();
        }

        let since = Utc::now() - TimeDelta::hours(1);
        let whose = Whose::Voter(who);

        let sitting_on_the_cap = store.ages_out_at(whose, since, 0).await.unwrap().unwrap();
        assert!(
            (sitting_on_the_cap - (Utc::now() - TimeDelta::minutes(50)))
                .abs()
                .num_seconds()
                < 2,
            "on the cap it is the oldest vote that has to go"
        );

        let two_over = store.ages_out_at(whose, since, 2).await.unwrap().unwrap();
        assert!(
            (two_over - (Utc::now() - TimeDelta::minutes(30)))
                .abs()
                .num_seconds()
                < 2,
            "two over the cap, the two oldest going is not enough: the third is the one that \
             actually frees a slot, and answering with the oldest would send them back early"
        );

        assert_eq!(
            store.ages_out_at(whose, since, 99).await.unwrap(),
            None,
            "asking past the end of the window is not an answer to invent"
        );
    }

    #[tokio::test]
    async fn a_cohort_budget_reaches_across_the_tokens_behind_it() {
        let store = store().await;

        for seed in 1..=3 {
            store
                .record(&cast(voter(seed), 0, 1, Outcome::Left), never())
                .await
                .unwrap();
        }

        let since = Utc::now() - TimeDelta::hours(1);
        assert_eq!(store.cohort_votes(b"cohort", since).await.unwrap(), 3);
        assert_eq!(store.cohort_votes(b"elsewhere", since).await.unwrap(), 0);

        let kin = store.kin(b"cohort").await.unwrap();
        assert_eq!(kin.len(), 3, "a troll who minted three tokens minted three");
    }

    #[tokio::test]
    async fn an_expired_cohort_is_dropped_and_the_voter_stays() {
        let store = store().await;
        let who = voter(1);
        store
            .record(&cast(who, 0, 1, Outcome::Left), never())
            .await
            .unwrap();

        assert_eq!(
            store
                .expire_cohorts(Utc::now() + TimeDelta::days(1))
                .await
                .unwrap(),
            1
        );
        let kept = store.voter(who).await.unwrap().unwrap();
        assert!(
            kept.cohort.is_none(),
            "the receipt outlives the abuse control"
        );
    }

    #[tokio::test]
    async fn a_voter_who_never_came_back_is_dropped_with_the_window() {
        let store = store().await;
        let (stale, active) = (voter(1), voter(2));

        let mut old = cast(stale, 0, 1, Outcome::Left);
        old.voted_at = Utc::now() - TimeDelta::days(200);
        store.record(&old, never()).await.unwrap();
        store
            .record(&cast(active, 0, 1, Outcome::Left), never())
            .await
            .unwrap();

        assert_eq!(
            store
                .forget_stale(Utc::now() - TimeDelta::days(90))
                .await
                .unwrap(),
            1
        );
        assert!(store.voter(stale).await.unwrap().is_none());
        assert!(store.voter(active).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_fit_is_cached_and_the_board_ranks_it_on_the_lower_bound() {
        let store = store().await;
        let who = voter(1);

        for _ in 0..3 {
            store
                .record(&cast(who, 0, 2, Outcome::Left), never())
                .await
                .unwrap();
        }
        for at in 0..300 {
            let outcome = if at % 5 < 3 {
                Outcome::Left
            } else {
                Outcome::Right
            };
            store
                .record(&cast(who, 1, 2, outcome), never())
                .await
                .unwrap();
        }

        let log = store.log(Category::Beautiful).await.unwrap();
        let fitted = fit(&log, &Grouping::default(), &Prior::weak());
        store.save(Category::Beautiful, &fitted).await.unwrap();

        let board = store
            .board(Category::Beautiful, MIN_COMPARISONS, 10, 0)
            .await
            .unwrap();
        assert_eq!(
            board.len(),
            2,
            "the unbeaten one has three comparisons and is off the board"
        );
        assert_eq!(board[0].score.picture, day(1));
        assert_eq!(board[0].inherited, 0.0, "nothing was inherited here");

        let tally = store.tally(Category::Beautiful).await.unwrap();
        assert_eq!(tally.votes, 303);
        assert_eq!(tally.model.as_deref(), Some(MODEL));
        assert!(tally.ran_at.is_some());
    }

    #[tokio::test]
    async fn refitting_replaces_the_cache_rather_than_piling_up() {
        let store = store().await;
        let who = voter(1);
        for _ in 0..10 {
            store
                .record(&cast(who, 0, 1, Outcome::Left), never())
                .await
                .unwrap();
        }

        let log = store.log(Category::Beautiful).await.unwrap();
        let fitted = fit(&log, &Grouping::default(), &Prior::weak());
        store.save(Category::Beautiful, &fitted).await.unwrap();
        store.save(Category::Beautiful, &fitted).await.unwrap();

        assert_eq!(store.scores(Category::Beautiful).await.unwrap().len(), 2);
        assert_eq!(
            store
                .board_size(Category::Beautiful, MIN_COMPARISONS)
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn evicting_a_troll_takes_their_picture_off_the_board_completely() {
        let store = store().await;
        let (honest, troll) = (voter(1), voter(2));

        for _ in 0..10 {
            store
                .record(&cast(honest, 0, 1, Outcome::Left), never())
                .await
                .unwrap();
        }
        for _ in 0..10 {
            store
                .record(&cast(troll, 5, 6, Outcome::Left), never())
                .await
                .unwrap();
        }

        async fn refit(store: &VoteStore) {
            let log = store.log(Category::Beautiful).await.unwrap();
            let fitted = fit(&log, &Grouping::default(), &Prior::weak());
            store.save(Category::Beautiful, &fitted).await.unwrap();
        }

        refit(&store).await;
        assert_eq!(store.scores(Category::Beautiful).await.unwrap().len(), 4);

        store.forget(troll).await.unwrap();
        refit(&store).await;
        let left = store.scores(Category::Beautiful).await.unwrap();
        assert_eq!(left.len(), 2, "no compensating entries and no residue");
        assert!(left.iter().all(|score| score.picture.days() < 2));
    }

    #[tokio::test]
    async fn a_baseline_survives_losing_the_database_and_live_votes_move_it() {
        let store = store().await;
        let dataset = Dataset::new(
            Category::Beautiful,
            vec![
                Row {
                    picture: day(0),
                    category: Category::Beautiful,
                    score: 1.8,
                    ess: 4_000.0,
                    comparisons: 9_000,
                },
                Row {
                    picture: day(1),
                    category: Category::Beautiful,
                    score: -0.4,
                    ess: 40.0,
                    comparisons: 60,
                },
            ],
        );

        assert_eq!(store.import(&dataset).await.unwrap(), 2);
        let anchors = store.anchors(Category::Beautiful).await.unwrap();
        assert_eq!(anchors.len(), 2);
        assert_eq!(
            anchors[0].ess,
            super::super::BASELINE_MAX_ESS,
            "capped on the way in"
        );

        let fitted = fit(&[], &Grouping::default(), &Prior::weak().anchored(anchors));
        store.save(Category::Beautiful, &fitted).await.unwrap();
        assert!((fitted.score(day(0)).unwrap().score - 1.8).abs() < 1e-6);

        assert_eq!(store.anchors(Category::Beautiful).await.unwrap().len(), 2);
        assert_eq!(store.scores(Category::Beautiful).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_board_restored_from_a_baseline_is_not_blank() {
        let store = store().await;
        let dataset = Dataset::new(
            Category::Beautiful,
            (0..4)
                .map(|at| Row {
                    picture: day(at),
                    category: Category::Beautiful,
                    score: 2.0 - f64::from(at),
                    // Two of them were well established in the previous life, two barely seen.
                    ess: if at < 2 { 40.0 } else { 2.0 },
                    comparisons: if at < 2 { 60 } else { 2 },
                })
                .collect(),
        );

        store.import(&dataset).await.unwrap();
        let anchors = store.anchors(Category::Beautiful).await.unwrap();
        let fitted = fit(&[], &Grouping::default(), &Prior::weak().anchored(anchors));
        store.save(Category::Beautiful, &fitted).await.unwrap();

        let board = store
            .board(Category::Beautiful, MIN_COMPARISONS, 10, 0)
            .await
            .unwrap();

        assert_eq!(
            board.len(),
            2,
            "a baseline that leaves the board empty has bought nothing anybody can see"
        );
        assert_eq!(board[0].score.picture, day(0));
        assert_eq!(board[0].score.comparisons, 0, "nobody here has voted yet");
        assert_eq!(board[0].inherited, 40.0);
        assert!(board[0].evidence() >= f64::from(MIN_COMPARISONS));
        assert_eq!(
            store
                .board_size(Category::Beautiful, MIN_COMPARISONS)
                .await
                .unwrap(),
            2,
            "and the thinly seen pair is still held back"
        );
    }

    #[tokio::test]
    async fn a_re_import_that_dropped_a_picture_drops_its_prior_too() {
        let store = store().await;
        let row = |date: i32, score: f64| Row {
            picture: day(date),
            category: Category::Beautiful,
            score,
            ess: 20.0,
            comparisons: 30,
        };

        store
            .import(&Dataset::new(
                Category::Beautiful,
                vec![row(0, 1.0), row(1, 0.5)],
            ))
            .await
            .unwrap();
        store
            .import(&Dataset::new(Category::Beautiful, vec![row(0, 1.0)]))
            .await
            .unwrap();

        let anchors = store.anchors(Category::Beautiful).await.unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].picture, day(0));
    }

    #[test]
    fn a_voter_id_survives_the_trip_through_a_cookie_and_a_forged_one_does_not() {
        let id = VoterId::new([
            0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a, 0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2,
            0xe1, 0xf0,
        ]);

        let hex = id.to_hex();
        assert_eq!(hex.len(), 32);
        assert_eq!(VoterId::from_hex(&hex), Some(id));

        assert_eq!(VoterId::from_hex(""), None);
        assert_eq!(VoterId::from_hex(&hex[..30]), None);
        assert_eq!(VoterId::from_hex(&format!("{hex}00")), None);
        assert_eq!(VoterId::from_hex(&"z".repeat(32)), None);
    }

    #[test]
    fn a_voter_id_does_not_write_itself_out_in_full_in_a_log_line() {
        let id = VoterId::new([0xab; VOTER_ID_BYTES]);
        let shown = format!("{id:?}");

        assert!(!shown.contains(&id.to_hex()), "{shown}");
        assert!(shown.starts_with("voter:ab"), "{shown}");
    }
}
