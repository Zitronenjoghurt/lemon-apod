use anyhow::{Context, Result};
use apod_core::ApodDate;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS fetches (
  date_id         INTEGER PRIMARY KEY,
  url             TEXT NOT NULL,
  http_status     INTEGER,
  sha256          TEXT,
  bytes           INTEGER,
  fetched_at      INTEGER,
  last_checked_at INTEGER,
  error           TEXT
);
CREATE INDEX IF NOT EXISTS idx_fetches_checked ON fetches(last_checked_at);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

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

pub struct ArchiveStore {
    conn: Connection,
}

impl ArchiveStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = super::db::open(path)?;
        conn.execute_batch(SCHEMA)
            .with_context(|| format!("initialising {}", path.display()))?;
        Ok(Self { conn })
    }

    pub fn get(&self, date: ApodDate) -> Result<Option<FetchRecord>> {
        self.conn
            .query_row(
                "SELECT http_status, sha256 FROM fetches WHERE date_id = ?1",
                params![date.days()],
                |row| {
                    Ok(FetchRecord {
                        http_status: row.get::<_, Option<i64>>(0)?.map(|status| status as u16),
                        sha256: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("reading fetch record")
    }

    pub fn record_success(
        &self,
        date: ApodDate,
        url: &str,
        sha256: &str,
        bytes: usize,
        now: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO fetches (date_id, url, http_status, sha256, bytes, fetched_at,
                                  last_checked_at, error)
             VALUES (?1, ?2, 200, ?3, ?4, ?5, ?5, NULL)
             ON CONFLICT(date_id) DO UPDATE SET
               url = excluded.url, http_status = 200, sha256 = excluded.sha256,
               bytes = excluded.bytes, fetched_at = excluded.fetched_at,
               last_checked_at = excluded.last_checked_at, error = NULL",
            params![date.days(), url, sha256, bytes as i64, now],
        )?;
        Ok(())
    }

    pub fn record_failure(
        &self,
        date: ApodDate,
        url: &str,
        status: Option<u16>,
        error: &str,
        now: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO fetches (date_id, url, http_status, last_checked_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(date_id) DO UPDATE SET
               url = excluded.url, http_status = excluded.http_status,
               last_checked_at = excluded.last_checked_at, error = excluded.error",
            params![date.days(), url, status.map(i64::from), now, error],
        )?;
        Ok(())
    }

    pub fn touch(&self, date: ApodDate, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE fetches SET last_checked_at = ?2, error = NULL WHERE date_id = ?1",
            params![date.days(), now],
        )?;
        Ok(())
    }

    pub fn next_target(&self, today: ApodDate) -> Result<Option<ApodDate>> {
        let attempted = self.attempted()?;

        if let Some(date) = today
            .iter_desc()
            .find(|date| !attempted.contains(&i64::from(date.days())))
        {
            return Ok(Some(date));
        }

        self.conn
            .query_row(
                "SELECT date_id FROM fetches
                 WHERE http_status IS NULL OR http_status NOT IN (200, 404)
                 ORDER BY last_checked_at ASC LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("looking for a retryable fetch")
            .map(|days| days.map(|d| ApodDate::from_days(d as i32)))
    }

    pub fn recheck_candidates(&self, limit: u32) -> Result<Vec<ApodDate>> {
        let mut stmt = self.conn.prepare(
            "SELECT date_id FROM fetches WHERE http_status = 200
             ORDER BY last_checked_at ASC LIMIT ?1",
        )?;
        let dates = stmt
            .query_map(params![limit], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|days| ApodDate::from_days(days as i32))
            .collect();
        Ok(dates)
    }

    pub fn counts(&self) -> Result<Counts> {
        self.conn
            .query_row(
                "SELECT
                   COUNT(*) FILTER (WHERE http_status = 200),
                   COUNT(*) FILTER (WHERE http_status = 404),
                   COUNT(*) FILTER (WHERE http_status IS NULL OR http_status NOT IN (200, 404)),
                   COALESCE(SUM(bytes), 0)
                 FROM fetches",
                [],
                |row| {
                    Ok(Counts {
                        stored: row.get(0)?,
                        absent: row.get(1)?,
                        failed: row.get(2)?,
                        bytes: row.get(3)?,
                    })
                },
            )
            .context("counting fetches")
    }

    fn attempted(&self) -> Result<HashSet<i64>> {
        let mut stmt = self.conn.prepare("SELECT date_id FROM fetches")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(ids)
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

    fn store() -> ArchiveStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        ArchiveStore { conn }
    }

    fn date(y: i32, m: u32, d: u32) -> ApodDate {
        ApodDate::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn backfill_walks_backwards_from_today() {
        let store = store();
        let today = date(1995, 6, 25);
        assert_eq!(store.next_target(today).unwrap(), Some(today));

        store.record_success(today, "u", "hash", 100, 1).unwrap();
        assert_eq!(
            store.next_target(today).unwrap(),
            Some(date(1995, 6, 24)),
            "should move to the next older date"
        );
    }

    #[test]
    fn a_failing_page_does_not_stall_the_walk() {
        let store = store();
        let today = date(1995, 6, 22);

        store
            .record_failure(today, "u", Some(500), "boom", 10)
            .unwrap();

        assert_eq!(store.next_target(today).unwrap(), Some(date(1995, 6, 21)));

        for day in [21, 20, 16] {
            store
                .record_success(date(1995, 6, day), "u", "h", 100, 1)
                .unwrap();
        }

        assert_eq!(store.next_target(today).unwrap(), Some(today));
    }

    #[test]
    fn known_gaps_are_never_targeted() {
        let store = store();
        let today = date(1995, 6, 20);

        store.record_success(today, "u", "h", 100, 1).unwrap();

        assert_eq!(store.next_target(today).unwrap(), Some(ApodDate::START));
    }

    #[test]
    fn records_absence_permanently() {
        let store = store();
        let missing = date(2020, 6, 10);
        store
            .record_failure(missing, "u", Some(404), "not found", 1)
            .unwrap();

        let record = store.get(missing).unwrap().unwrap();
        assert!(record.is_absent());
        assert!(!record.is_success());
    }

    #[test]
    fn counts_by_outcome() {
        let store = store();
        store
            .record_success(date(2020, 1, 1), "u", "h", 4096, 1)
            .unwrap();
        store
            .record_failure(date(2020, 1, 2), "u", Some(404), "gone", 1)
            .unwrap();
        store
            .record_failure(date(2020, 1, 3), "u", Some(500), "boom", 1)
            .unwrap();

        let counts = store.counts().unwrap();
        assert_eq!((counts.stored, counts.absent, counts.failed), (1, 1, 1));
        assert_eq!(counts.bytes, 4096);
    }
}
