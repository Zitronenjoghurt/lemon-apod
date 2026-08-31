use anyhow::{Context, Result};
use apod_core::ApodDate;
use apod_core::db::{Db, DbConfig};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::migrate::Migrator;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

const BACKOFF_BASE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Legacy,
    Modern,
}

impl Source {
    pub const ALL: [Self; 2] = [Self::Legacy, Self::Modern];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Modern => "modern",
        }
    }

    fn legacy() -> Self {
        Self::Legacy
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn backoff(attempts: u32, ceiling: Duration) -> Duration {
    BACKOFF_BASE
        .saturating_mul(1u32 << attempts.saturating_sub(1).min(31))
        .min(ceiling)
}

#[derive(Debug, Clone, Copy)]
struct Attempted {
    date_id: i64,
    http_status: Option<u16>,
    attempts: u32,
    last_checked_at: i64,
}

impl Attempted {
    fn settled(self) -> bool {
        matches!(self.http_status, Some(200 | 404 | 410 | 300..=399))
    }

    fn due_in(self, backoff_max: Duration, now: i64) -> i64 {
        backoff(self.attempts, backoff_max).as_secs() as i64
            - now.saturating_sub(self.last_checked_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next<T> {
    Fetch(T),
    Waiting(Duration),
    Complete,
}

#[derive(Debug, Clone)]
pub struct FetchRecord {
    pub http_status: Option<u16>,
    pub sha256: Option<String>,
}

impl FetchRecord {
    pub fn is_success(&self) -> bool {
        self.http_status == Some(200)
    }
    pub fn is_absent(&self) -> bool {
        self.http_status == Some(404)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Failure<'a> {
    pub status: Option<u16>,
    pub final_url: Option<&'a str>,
    pub error: &'a str,
}

impl<'a> Failure<'a> {
    pub fn new(status: Option<u16>, error: &'a str) -> Self {
        Self {
            status,
            final_url: None,
            error,
        }
    }

    pub fn landed_on(mut self, url: Option<&'a str>) -> Self {
        self.final_url = url;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRow {
    pub date: ApodDate,
    #[serde(default = "Source::legacy")]
    pub source: Source,
    pub url: String,
    pub http_status: Option<u16>,
    pub sha256: Option<String>,
    pub bytes: Option<i64>,
    pub fetched_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ArchiveStore {
    db: Db,
}

impl ArchiveStore {
    pub async fn open(path: &Path) -> Result<Self> {
        let db = Db::open(DbConfig::read_write(path))
            .await
            .with_context(|| format!("opening {}", path.display()))?;
        db.migrate(&MIGRATIONS)
            .await
            .with_context(|| format!("migrating {}", path.display()))?;
        Ok(Self { db })
    }

    pub fn media(&self) -> crate::media::MediaStore {
        crate::media::MediaStore::new(self.db.clone())
    }

    pub async fn get(&self, date: ApodDate, source: Source) -> Result<Option<FetchRecord>> {
        let row: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT http_status, sha256 FROM fetches WHERE date_id = ?1 AND source = ?2",
        )
        .bind(date.days())
        .bind(source.as_str())
        .fetch_optional(self.db.reader())
        .await
        .context("reading fetch record")?;

        Ok(row.map(|(http_status, sha256)| FetchRecord {
            http_status: http_status.map(|status| status as u16),
            sha256,
        }))
    }

    pub async fn record_success(
        &self,
        date: ApodDate,
        source: Source,
        url: &str,
        sha256: &str,
        bytes: usize,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fetches (date_id, source, url, final_url, http_status, sha256, bytes,
                                  fetched_at, last_checked_at, attempts, error)
             VALUES (?1, ?2, ?3, NULL, 200, ?4, ?5, ?6, ?6, 0, NULL)
             ON CONFLICT(date_id, source) DO UPDATE SET
               url = excluded.url, final_url = NULL, http_status = 200,
               sha256 = excluded.sha256, bytes = excluded.bytes,
               fetched_at = excluded.fetched_at, last_checked_at = excluded.last_checked_at,
               attempts = 0, error = NULL",
        )
        .bind(date.days())
        .bind(source.as_str())
        .bind(url)
        .bind(sha256)
        .bind(bytes as i64)
        .bind(now)
        .execute(self.db.writer()?)
        .await
        .context("recording a successful fetch")?;
        Ok(())
    }

    pub async fn record_failure(
        &self,
        date: ApodDate,
        source: Source,
        url: &str,
        failure: Failure<'_>,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fetches (date_id, source, url, final_url, http_status, last_checked_at,
                                  attempts, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
             ON CONFLICT(date_id, source) DO UPDATE SET
               url = excluded.url, final_url = excluded.final_url,
               http_status = excluded.http_status,
               last_checked_at = excluded.last_checked_at, attempts = fetches.attempts + 1,
               error = excluded.error",
        )
        .bind(date.days())
        .bind(source.as_str())
        .bind(url)
        .bind(failure.final_url)
        .bind(failure.status.map(i64::from))
        .bind(now)
        .bind(failure.error)
        .execute(self.db.writer()?)
        .await
        .context("recording a failed fetch")?;
        Ok(())
    }

    pub async fn touch(&self, date: ApodDate, source: Source, now: i64) -> Result<()> {
        sqlx::query(
            "UPDATE fetches SET last_checked_at = ?3, attempts = 0, error = NULL
             WHERE date_id = ?1 AND source = ?2",
        )
        .bind(date.days())
        .bind(source.as_str())
        .bind(now)
        .execute(self.db.writer()?)
        .await
        .context("touching a fetch record")?;
        Ok(())
    }

    pub async fn touch_between(
        &self,
        source: Source,
        from: ApodDate,
        to: ApodDate,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE fetches SET last_checked_at = ?1
             WHERE source = ?2 AND date_id BETWEEN ?3 AND ?4",
        )
        .bind(now)
        .bind(source.as_str())
        .bind(from.days())
        .bind(to.days())
        .execute(self.db.writer()?)
        .await
        .context("touching a range of fetch records")?;
        Ok(())
    }

    pub async fn stale_before(&self, source: Source, cutoff: i64) -> Result<Option<ApodDate>> {
        let newest: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(date_id) FROM fetches WHERE source = ?1 AND last_checked_at < ?2",
        )
        .bind(source.as_str())
        .bind(cutoff)
        .fetch_one(self.db.reader())
        .await
        .context("looking for the newest date due a re-check")?;

        Ok(newest.map(|days| ApodDate::from_days(days as i32)))
    }

    pub async fn owed(&self, today: ApodDate, source: Source) -> Result<usize> {
        let seen: HashSet<i64> = self
            .attempted(source)
            .await?
            .iter()
            .map(|row| row.date_id)
            .collect();

        Ok(today
            .iter_desc()
            .filter(|date| !seen.contains(&i64::from(date.days())))
            .count())
    }

    pub async fn next_target(
        &self,
        today: ApodDate,
        source: Source,
        backoff_max: Duration,
        now: i64,
    ) -> Result<Next<ApodDate>> {
        let attempted = self.attempted(source).await?;
        let seen: HashSet<i64> = attempted.iter().map(|row| row.date_id).collect();

        if let Some(date) = today
            .iter_desc()
            .find(|date| !seen.contains(&i64::from(date.days())))
        {
            return Ok(Next::Fetch(date));
        }

        let mut soonest: Option<i64> = None;
        for row in attempted.iter().filter(|row| !row.settled()) {
            let due_in = row.due_in(backoff_max, now);
            if due_in <= 0 {
                return Ok(Next::Fetch(ApodDate::from_days(row.date_id as i32)));
            }
            soonest = Some(soonest.map_or(due_in, |soonest: i64| soonest.min(due_in)));
        }

        Ok(match soonest {
            Some(seconds) => Next::Waiting(Duration::from_secs(seconds as u64)),
            None => Next::Complete,
        })
    }

    pub async fn recorded_between(
        &self,
        source: Source,
        from: ApodDate,
        to: ApodDate,
    ) -> Result<HashSet<ApodDate>> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM fetches WHERE source = ?1 AND date_id BETWEEN ?2 AND ?3",
        )
        .bind(source.as_str())
        .bind(from.days())
        .bind(to.days())
        .fetch_all(self.db.reader())
        .await
        .context("reading which dates are already recorded")?;

        Ok(rows
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect())
    }

    pub async fn recheck_candidates(&self, source: Source, limit: u32) -> Result<Vec<ApodDate>> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM fetches WHERE source = ?1 AND http_status = 200
             ORDER BY last_checked_at ASC LIMIT ?2",
        )
        .bind(source.as_str())
        .bind(limit)
        .fetch_all(self.db.reader())
        .await
        .context("listing recheck candidates")?;

        Ok(rows
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect())
    }

    pub async fn fetch_rows(&self, source: Source) -> Result<Vec<FetchRow>> {
        let rows = sqlx::query(
            "SELECT date_id, url, http_status, sha256, bytes, fetched_at
             FROM fetches WHERE source = ?1 ORDER BY date_id",
        )
        .bind(source.as_str())
        .fetch_all(self.db.reader())
        .await
        .context("reading fetch state")?;

        Ok(rows
            .iter()
            .map(|row| FetchRow {
                date: ApodDate::from_days(row.get::<i64, _>("date_id") as i32),
                source,
                url: row.get("url"),
                http_status: row
                    .get::<Option<i64>, _>("http_status")
                    .map(|status| status as u16),
                sha256: row.get("sha256"),
                bytes: row.get("bytes"),
                fetched_at: row.get("fetched_at"),
            })
            .collect())
    }

    pub async fn seed(&self, rows: &[FetchRow]) -> Result<usize> {
        let mut tx = self
            .db
            .writer()?
            .begin()
            .await
            .context("opening the seed transaction")?;
        let mut seeded = 0;

        for row in rows {
            let done = sqlx::query(
                "INSERT INTO fetches (date_id, source, url, http_status, sha256, bytes,
                                      fetched_at, last_checked_at, attempts, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0, NULL)
                 ON CONFLICT(date_id, source) DO NOTHING",
            )
            .bind(row.date.days())
            .bind(row.source.as_str())
            .bind(&row.url)
            .bind(row.http_status.map(i64::from))
            .bind(&row.sha256)
            .bind(row.bytes)
            .bind(row.fetched_at)
            .execute(&mut *tx)
            .await
            .context("seeding fetch state")?;
            seeded += done.rows_affected() as usize;
        }

        tx.commit().await.context("committing seeded fetch state")?;
        Ok(seeded)
    }

    pub async fn counts(&self, source: Source) -> Result<Counts> {
        let (stored, absent, redirected, failed, bytes): (i64, i64, i64, i64, i64) =
            sqlx::query_as(
                "SELECT
                   COUNT(*) FILTER (WHERE http_status = 200),
                   COUNT(*) FILTER (WHERE http_status IN (404, 410)),
                   COUNT(*) FILTER (WHERE http_status BETWEEN 300 AND 399),
                   COUNT(*) FILTER (WHERE COALESCE(http_status, 0) NOT IN (200, 404, 410)
                                      AND COALESCE(http_status, 0) NOT BETWEEN 300 AND 399),
                   COALESCE(SUM(bytes), 0)
                 FROM fetches WHERE source = ?1",
            )
            .bind(source.as_str())
            .fetch_one(self.db.reader())
            .await
            .context("counting fetches")?;

        Ok(Counts {
            stored,
            absent,
            redirected,
            failed,
            bytes,
        })
    }

    pub async fn stored_dates(&self) -> Result<i64> {
        sqlx::query_scalar("SELECT COUNT(DISTINCT date_id) FROM fetches WHERE http_status = 200")
            .fetch_one(self.db.reader())
            .await
            .context("counting stored dates")
    }

    async fn attempted(&self, source: Source) -> Result<Vec<Attempted>> {
        let rows = sqlx::query(
            "SELECT date_id, http_status, attempts, last_checked_at FROM fetches
             WHERE source = ?1 ORDER BY last_checked_at ASC",
        )
        .bind(source.as_str())
        .fetch_all(self.db.reader())
        .await
        .context("listing attempted dates")?;

        Ok(rows
            .iter()
            .map(|row| Attempted {
                date_id: row.get("date_id"),
                http_status: row
                    .get::<Option<i64>, _>("http_status")
                    .map(|status| status as u16),
                attempts: row.get::<i64, _>("attempts") as u32,
                last_checked_at: row
                    .get::<Option<i64>, _>("last_checked_at")
                    .unwrap_or_default(),
            })
            .collect())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Counts {
    pub stored: i64,
    pub absent: i64,
    pub redirected: i64,
    pub failed: i64,
    pub bytes: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const NEVER: Duration = Duration::ZERO;
    const CEILING: Duration = Duration::from_secs(6 * 3600);
    const LATER: i64 = i64::MAX / 2;

    async fn store() -> ArchiveStore {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "apod-archive-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ))
            .join("archive.db");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        ArchiveStore::open(&path).await.unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> ApodDate {
        ApodDate::from_ymd(y, m, d).unwrap()
    }

    async fn target(store: &ArchiveStore, today: ApodDate) -> Next<ApodDate> {
        store
            .next_target(today, Source::Legacy, NEVER, LATER)
            .await
            .unwrap()
    }

    async fn target_at(store: &ArchiveStore, today: ApodDate, now: i64) -> Next<ApodDate> {
        store
            .next_target(today, Source::Legacy, CEILING, now)
            .await
            .unwrap()
    }

    async fn succeed(store: &ArchiveStore, date: ApodDate, now: i64) {
        store
            .record_success(date, Source::Legacy, "u", "h", 100, now)
            .await
            .unwrap();
    }

    async fn stale(store: &ArchiveStore, cutoff: i64) -> Option<ApodDate> {
        store.stale_before(Source::Legacy, cutoff).await.unwrap()
    }

    #[tokio::test]
    async fn a_rolling_recheck_starts_at_the_newest_date_that_has_gone_unchecked() {
        let store = store().await;
        succeed(&store, date(2011, 3, 1), 100).await;
        succeed(&store, date(2011, 3, 20), 100).await;
        succeed(&store, date(2011, 3, 10), 900).await;

        assert_eq!(
            stale(&store, 500).await,
            Some(date(2011, 3, 20)),
            "the newest of the two nothing has looked at since the cutoff"
        );
        assert_eq!(
            stale(&store, 50).await,
            None,
            "nothing is overdue, so there is no window to ask for"
        );
        assert_eq!(
            stale(&store, 1000).await,
            Some(date(2011, 3, 20)),
            "the walk goes newest first however recently each one was read"
        );
    }

    #[tokio::test]
    async fn a_date_the_window_carried_nothing_for_still_counts_as_looked_at() {
        let store = store().await;
        let absent = date(2011, 3, 5);
        fail(&store, absent, 404, 100).await;
        succeed(&store, date(2011, 3, 20), 100).await;

        store
            .touch_between(Source::Legacy, date(2011, 3, 1), date(2011, 3, 31), 900)
            .await
            .unwrap();

        assert_eq!(
            stale(&store, 500).await,
            None,
            "a date the collection holds nothing for would otherwise read as overdue for good, \
             and the walk would never get past it"
        );

        let row = store.get(absent, Source::Legacy).await.unwrap().unwrap();
        assert_eq!(
            row.http_status,
            Some(404),
            "only the timestamp moves; why it is absent is still on the record"
        );
    }

    async fn fail(store: &ArchiveStore, date: ApodDate, status: u16, now: i64) {
        store
            .record_failure(
                date,
                Source::Legacy,
                "u",
                Failure::new(Some(status), "boom"),
                now,
            )
            .await
            .unwrap();
    }

    async fn attempts(store: &ArchiveStore, date: ApodDate) -> i64 {
        sqlx::query_scalar("SELECT attempts FROM fetches WHERE date_id = ?1 AND source = 'legacy'")
            .bind(date.days())
            .fetch_one(store.db.reader())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn owed_counts_the_dates_never_asked_for() {
        let store = store().await;
        let today = date(1995, 6, 25);

        let all = store.owed(today, Source::Legacy).await.unwrap();
        assert_eq!(
            all,
            today.iter_desc().count(),
            "nothing has been asked for yet"
        );

        succeed(&store, today, 1).await;
        fail(&store, date(1995, 6, 24), 500, 1).await;
        assert_eq!(
            store.owed(today, Source::Legacy).await.unwrap(),
            all - 2,
            "a date that failed has still been asked for, so it is no longer owed a first ask"
        );

        assert_eq!(
            store.owed(today, Source::Modern).await.unwrap(),
            all,
            "the two sources are counted apart"
        );
    }

    #[tokio::test]
    async fn backfill_walks_backwards_from_today() {
        let store = store().await;
        let today = date(1995, 6, 25);
        assert_eq!(target(&store, today).await, Next::Fetch(today));

        succeed(&store, today, 1).await;
        assert_eq!(
            target(&store, today).await,
            Next::Fetch(date(1995, 6, 24)),
            "should move to the next older date"
        );
    }

    #[tokio::test]
    async fn a_failing_page_does_not_stall_the_walk() {
        let store = store().await;
        let today = date(1995, 6, 22);

        fail(&store, today, 500, 10).await;

        assert_eq!(target(&store, today).await, Next::Fetch(date(1995, 6, 21)));

        for day in [21, 20, 16] {
            succeed(&store, date(1995, 6, day), 1).await;
        }

        assert_eq!(target(&store, today).await, Next::Fetch(today));
    }

    #[tokio::test]
    async fn known_gaps_are_never_targeted() {
        let store = store().await;
        let today = date(1995, 6, 20);

        succeed(&store, today, 1).await;

        assert_eq!(target(&store, today).await, Next::Fetch(ApodDate::START));
    }

    #[tokio::test]
    async fn records_absence_permanently() {
        let store = store().await;
        let missing = date(2020, 6, 10);
        store
            .record_failure(
                missing,
                Source::Legacy,
                "u",
                Failure::new(Some(404), "not found"),
                1,
            )
            .await
            .unwrap();

        let record = store.get(missing, Source::Legacy).await.unwrap().unwrap();
        assert!(record.is_absent());
        assert!(!record.is_success());
    }

    #[tokio::test]
    async fn the_two_sources_are_independent_for_one_date() {
        let store = store().await;
        let day = ApodDate::START;

        succeed(&store, day, 1).await;
        store
            .record_failure(
                day,
                Source::Modern,
                "https://science.nasa.gov/api",
                Failure::new(Some(500), "boom"),
                1,
            )
            .await
            .unwrap();

        let legacy = store.get(day, Source::Legacy).await.unwrap().unwrap();
        let modern = store.get(day, Source::Modern).await.unwrap().unwrap();
        assert!(legacy.is_success());
        assert_eq!(modern.http_status, Some(500));

        assert_eq!(store.counts(Source::Legacy).await.unwrap().stored, 1);
        assert_eq!(store.counts(Source::Modern).await.unwrap().stored, 0);
        assert_eq!(
            store.stored_dates().await.unwrap(),
            1,
            "two rows for one date are still one date"
        );

        assert_eq!(
            target_at(&store, day, LATER).await,
            Next::Complete,
            "the legacy side of this date is stored and done"
        );
        assert_eq!(
            store
                .next_target(day, Source::Modern, CEILING, LATER)
                .await
                .unwrap(),
            Next::Fetch(day),
            "the modern side failed and is still worth another request"
        );
    }

    #[tokio::test]
    async fn only_a_transient_failure_is_ever_retried() {
        let store = store().await;
        let today = date(1995, 6, 22);

        succeed(&store, today, 1).await;
        fail(&store, date(1995, 6, 21), 404, 2).await;
        fail(&store, date(1995, 6, 20), 301, 3).await;

        assert_eq!(
            target_at(&store, today, LATER).await,
            Next::Fetch(ApodDate::START),
            "the one date nobody has tried yet"
        );

        fail(&store, ApodDate::START, 410, 4).await;
        assert_eq!(
            target_at(&store, today, LATER).await,
            Next::Complete,
            "stored, absent, gone and redirected are all settled for good"
        );

        fail(&store, date(1995, 6, 21), 503, 5).await;
        assert_eq!(
            target_at(&store, today, LATER).await,
            Next::Fetch(date(1995, 6, 21)),
            "a status that could change is the only kind worth another request"
        );
    }

    #[tokio::test]
    async fn a_redirect_records_where_it_landed_and_stops_targeting_the_date() {
        let store = store().await;
        let today = ApodDate::START;

        store
            .record_failure(
                today,
                Source::Legacy,
                "https://apod.nasa.gov/apod/ap950616.html",
                Failure::new(Some(301), "redirected with 301")
                    .landed_on(Some("https://science.nasa.gov/apod/")),
                1,
            )
            .await
            .unwrap();

        assert_eq!(
            target_at(&store, today, LATER).await,
            Next::Complete,
            "however long it waits, the legacy host is not coming back"
        );

        let landed: Option<String> = sqlx::query_scalar(
            "SELECT final_url FROM fetches WHERE date_id = ?1 AND source = 'legacy'",
        )
        .bind(today.days())
        .fetch_one(store.db.reader())
        .await
        .unwrap();
        assert_eq!(landed.as_deref(), Some("https://science.nasa.gov/apod/"));
    }

    #[test]
    fn the_backoff_doubles_up_to_the_ceiling() {
        assert_eq!(backoff(0, CEILING), BACKOFF_BASE);
        assert_eq!(backoff(1, CEILING), BACKOFF_BASE);
        assert_eq!(backoff(2, CEILING), BACKOFF_BASE * 2);
        assert_eq!(backoff(5, CEILING), BACKOFF_BASE * 16);
        assert_eq!(backoff(30, CEILING), CEILING);
        assert_eq!(backoff(u32::MAX, CEILING), CEILING);
    }

    #[tokio::test]
    async fn a_failing_date_waits_out_its_backoff() {
        let store = store().await;
        let today = ApodDate::START;

        fail(&store, today, 500, 1_000).await;
        assert_eq!(
            target_at(&store, today, 1_030).await,
            Next::Waiting(Duration::from_secs(30)),
            "30 seconds into the first backoff, with 30 to go"
        );
        assert_eq!(target_at(&store, today, 1_060).await, Next::Fetch(today));

        fail(&store, today, 500, 2_000).await;
        assert_eq!(attempts(&store, today).await, 2);
        assert_eq!(
            target_at(&store, today, 2_060).await,
            Next::Waiting(Duration::from_secs(60)),
            "a second failure doubles the wait"
        );
        assert_eq!(target_at(&store, today, 2_120).await, Next::Fetch(today));
    }

    #[tokio::test]
    async fn attempts_reset_on_success() {
        let store = store().await;
        let target = date(2020, 1, 1);

        fail(&store, target, 500, 1).await;
        fail(&store, target, 500, 2).await;
        assert_eq!(attempts(&store, target).await, 2);

        succeed(&store, target, 3).await;
        assert_eq!(attempts(&store, target).await, 0);

        fail(&store, target, 500, 4).await;
        store.touch(target, Source::Legacy, 5).await.unwrap();
        assert_eq!(
            attempts(&store, target).await,
            0,
            "a page that came back unchanged was fetched successfully"
        );
    }

    #[tokio::test]
    async fn counts_by_outcome() {
        let store = store().await;
        store
            .record_success(date(2020, 1, 1), Source::Legacy, "u", "h", 4096, 1)
            .await
            .unwrap();
        fail(&store, date(2020, 1, 2), 404, 1).await;
        fail(&store, date(2020, 1, 3), 410, 1).await;
        fail(&store, date(2020, 1, 4), 301, 1).await;
        fail(&store, date(2020, 1, 5), 500, 1).await;

        let counts = store.counts(Source::Legacy).await.unwrap();
        assert_eq!(
            (
                counts.stored,
                counts.absent,
                counts.redirected,
                counts.failed
            ),
            (1, 2, 1, 1)
        );
        assert_eq!(counts.bytes, 4096);
    }

    #[tokio::test]
    async fn exports_and_seeds_only_the_source_that_was_asked_for() {
        let both = store().await;
        let day = date(2020, 1, 1);

        succeed(&both, day, 1).await;
        both.record_success(
            day,
            Source::Modern,
            "https://science.nasa.gov/api",
            "j",
            10,
            1,
        )
        .await
        .unwrap();

        let rows = both.fetch_rows(Source::Legacy).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, Source::Legacy);
        assert_eq!(rows[0].url, "u");

        let restored = store().await;
        assert_eq!(restored.seed(&rows).await.unwrap(), 1);
        assert!(
            restored
                .get(day, Source::Legacy)
                .await
                .unwrap()
                .unwrap()
                .is_success()
        );
        assert!(restored.get(day, Source::Modern).await.unwrap().is_none());
    }

    #[test]
    fn a_fetch_row_without_a_source_is_legacy() {
        let row: FetchRow = serde_json::from_str(
            r#"{"date":"2024-03-05","url":"u","http_status":200,"sha256":"h","bytes":10,
                "fetched_at":1}"#,
        )
        .unwrap();
        assert_eq!(row.source, Source::Legacy);

        let round_tripped: FetchRow =
            serde_json::from_str(&serde_json::to_string(&row).unwrap()).unwrap();
        assert_eq!(round_tripped.source, Source::Legacy);
    }

    #[tokio::test]
    async fn adopts_a_database_that_predates_migrations() {
        let dir = std::env::temp_dir().join(format!("apod-archive-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("archive.db");

        let legacy = Db::open(DbConfig::read_write(&path)).await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE fetches (
               date_id INTEGER PRIMARY KEY, url TEXT NOT NULL, http_status INTEGER,
               sha256 TEXT, bytes INTEGER, fetched_at INTEGER, last_checked_at INTEGER,
               error TEXT);
             CREATE INDEX idx_fetches_checked ON fetches(last_checked_at);
             CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO fetches (date_id, url, http_status, sha256, bytes, fetched_at,
                                  last_checked_at)
             VALUES (10486, 'u', 200, 'deadbeef', 4096, 1, 1),
                    (10487, 'u', 404, NULL, NULL, NULL, 1),
                    (10488, 'u', 500, NULL, NULL, NULL, 1);",
        )
        .execute(legacy.writer().unwrap())
        .await
        .unwrap();
        legacy.close().await;

        let store = ArchiveStore::open(&path).await.unwrap();
        let record = store
            .get(ApodDate::from_days(10486), Source::Legacy)
            .await
            .unwrap()
            .expect("the pre-existing row must survive");
        assert!(record.is_success());
        assert_eq!(record.sha256.as_deref(), Some("deadbeef"));

        let rows: Vec<(i64, String, i64)> =
            sqlx::query_as("SELECT date_id, source, attempts FROM fetches ORDER BY date_id")
                .fetch_all(store.db.reader())
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                (10486, "legacy".to_owned(), 0),
                (10487, "legacy".to_owned(), 1),
                (10488, "legacy".to_owned(), 1),
            ],
            "every row survives as legacy, and one that failed has been attempted once"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
