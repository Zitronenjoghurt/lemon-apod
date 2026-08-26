use anyhow::{Context, Result};
use apod_core::ApodDate;
use apod_core::db::{Db, DbConfig};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::migrate::Migrator;
use std::collections::HashSet;
use std::path::Path;

static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRow {
    pub date: ApodDate,
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

    pub async fn get(&self, date: ApodDate) -> Result<Option<FetchRecord>> {
        let row: Option<(Option<i64>, Option<String>)> =
            sqlx::query_as("SELECT http_status, sha256 FROM fetches WHERE date_id = ?1")
                .bind(date.days())
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
        url: &str,
        sha256: &str,
        bytes: usize,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fetches (date_id, url, http_status, sha256, bytes, fetched_at,
                                  last_checked_at, error)
             VALUES (?1, ?2, 200, ?3, ?4, ?5, ?5, NULL)
             ON CONFLICT(date_id) DO UPDATE SET
               url = excluded.url, http_status = 200, sha256 = excluded.sha256,
               bytes = excluded.bytes, fetched_at = excluded.fetched_at,
               last_checked_at = excluded.last_checked_at, error = NULL",
        )
        .bind(date.days())
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
        url: &str,
        status: Option<u16>,
        error: &str,
        now: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fetches (date_id, url, http_status, last_checked_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(date_id) DO UPDATE SET
               url = excluded.url, http_status = excluded.http_status,
               last_checked_at = excluded.last_checked_at, error = excluded.error",
        )
        .bind(date.days())
        .bind(url)
        .bind(status.map(i64::from))
        .bind(now)
        .bind(error)
        .execute(self.db.writer()?)
        .await
        .context("recording a failed fetch")?;
        Ok(())
    }

    pub async fn touch(&self, date: ApodDate, now: i64) -> Result<()> {
        sqlx::query("UPDATE fetches SET last_checked_at = ?2, error = NULL WHERE date_id = ?1")
            .bind(date.days())
            .bind(now)
            .execute(self.db.writer()?)
            .await
            .context("touching a fetch record")?;
        Ok(())
    }

    pub async fn next_target(&self, today: ApodDate) -> Result<Option<ApodDate>> {
        let attempted = self.attempted().await?;

        if let Some(date) = today
            .iter_desc()
            .find(|date| !attempted.contains(&i64::from(date.days())))
        {
            return Ok(Some(date));
        }

        let retryable: Option<i64> = sqlx::query_scalar(
            "SELECT date_id FROM fetches
             WHERE http_status IS NULL OR http_status NOT IN (200, 404)
             ORDER BY last_checked_at ASC LIMIT 1",
        )
        .fetch_optional(self.db.reader())
        .await
        .context("looking for a retryable fetch")?;

        Ok(retryable.map(|days| ApodDate::from_days(days as i32)))
    }

    pub async fn recheck_candidates(&self, limit: u32) -> Result<Vec<ApodDate>> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT date_id FROM fetches WHERE http_status = 200
             ORDER BY last_checked_at ASC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(self.db.reader())
        .await
        .context("listing recheck candidates")?;

        Ok(rows
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect())
    }

    pub async fn fetch_rows(&self) -> Result<Vec<FetchRow>> {
        let rows = sqlx::query(
            "SELECT date_id, url, http_status, sha256, bytes, fetched_at
             FROM fetches ORDER BY date_id",
        )
        .fetch_all(self.db.reader())
        .await
        .context("reading fetch state")?;

        Ok(rows
            .iter()
            .map(|row| FetchRow {
                date: ApodDate::from_days(row.get::<i64, _>("date_id") as i32),
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
                "INSERT INTO fetches (date_id, url, http_status, sha256, bytes, fetched_at,
                                      last_checked_at, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL)
                 ON CONFLICT(date_id) DO NOTHING",
            )
            .bind(row.date.days())
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

    pub async fn counts(&self) -> Result<Counts> {
        let (stored, absent, failed, bytes): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               COUNT(*) FILTER (WHERE http_status = 200),
               COUNT(*) FILTER (WHERE http_status = 404),
               COUNT(*) FILTER (WHERE http_status IS NULL OR http_status NOT IN (200, 404)),
               COALESCE(SUM(bytes), 0)
             FROM fetches",
        )
        .fetch_one(self.db.reader())
        .await
        .context("counting fetches")?;

        Ok(Counts {
            stored,
            absent,
            failed,
            bytes,
        })
    }

    async fn attempted(&self) -> Result<HashSet<i64>> {
        let ids: Vec<i64> = sqlx::query_scalar("SELECT date_id FROM fetches")
            .fetch_all(self.db.reader())
            .await
            .context("listing attempted dates")?;
        Ok(ids.into_iter().collect())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Counts {
    pub stored: i64,
    pub absent: i64,
    pub failed: i64,
    pub bytes: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

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

    #[tokio::test]
    async fn backfill_walks_backwards_from_today() {
        let store = store().await;
        let today = date(1995, 6, 25);
        assert_eq!(store.next_target(today).await.unwrap(), Some(today));

        store
            .record_success(today, "u", "hash", 100, 1)
            .await
            .unwrap();
        assert_eq!(
            store.next_target(today).await.unwrap(),
            Some(date(1995, 6, 24)),
            "should move to the next older date"
        );
    }

    #[tokio::test]
    async fn a_failing_page_does_not_stall_the_walk() {
        let store = store().await;
        let today = date(1995, 6, 22);

        store
            .record_failure(today, "u", Some(500), "boom", 10)
            .await
            .unwrap();

        assert_eq!(
            store.next_target(today).await.unwrap(),
            Some(date(1995, 6, 21))
        );

        for day in [21, 20, 16] {
            store
                .record_success(date(1995, 6, day), "u", "h", 100, 1)
                .await
                .unwrap();
        }

        assert_eq!(store.next_target(today).await.unwrap(), Some(today));
    }

    #[tokio::test]
    async fn known_gaps_are_never_targeted() {
        let store = store().await;
        let today = date(1995, 6, 20);

        store.record_success(today, "u", "h", 100, 1).await.unwrap();

        assert_eq!(
            store.next_target(today).await.unwrap(),
            Some(ApodDate::START)
        );
    }

    #[tokio::test]
    async fn records_absence_permanently() {
        let store = store().await;
        let missing = date(2020, 6, 10);
        store
            .record_failure(missing, "u", Some(404), "not found", 1)
            .await
            .unwrap();

        let record = store.get(missing).await.unwrap().unwrap();
        assert!(record.is_absent());
        assert!(!record.is_success());
    }

    #[tokio::test]
    async fn counts_by_outcome() {
        let store = store().await;
        store
            .record_success(date(2020, 1, 1), "u", "h", 4096, 1)
            .await
            .unwrap();
        store
            .record_failure(date(2020, 1, 2), "u", Some(404), "gone", 1)
            .await
            .unwrap();
        store
            .record_failure(date(2020, 1, 3), "u", Some(500), "boom", 1)
            .await
            .unwrap();

        let counts = store.counts().await.unwrap();
        assert_eq!((counts.stored, counts.absent, counts.failed), (1, 1, 1));
        assert_eq!(counts.bytes, 4096);
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
             VALUES (10486, 'u', 200, 'deadbeef', 4096, 1, 1);",
        )
        .execute(legacy.writer().unwrap())
        .await
        .unwrap();
        legacy.close().await;

        let store = ArchiveStore::open(&path).await.unwrap();
        let record = store
            .get(ApodDate::from_days(10486))
            .await
            .unwrap()
            .expect("the pre-existing row must survive");
        assert!(record.is_success());
        assert_eq!(record.sha256.as_deref(), Some("deadbeef"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
