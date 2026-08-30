use apod_core::db::{Db, DbConfig};
use apod_core::{ApodDate, ApodReader};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct Split {
    pub carried: i64,
    pub absent: i64,
    pub unchecked: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Coverage {
    pub entries: i64,
    #[serde(flatten)]
    pub split: Split,
    pub absent_dates: Vec<ApodDate>,
}

const ABSENT_LISTED: usize = 200;

#[derive(Clone)]
pub struct Archive {
    path: Arc<PathBuf>,
    db: Arc<RwLock<Option<Db>>>,
}

impl Archive {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Arc::new(path),
            db: Arc::new(RwLock::new(None)),
        }
    }

    async fn db(&self) -> Option<Db> {
        if let Some(db) = self.db.read().await.as_ref() {
            return Some(db.clone());
        }

        let mut held = self.db.write().await;
        if let Some(db) = held.as_ref() {
            return Some(db.clone());
        }

        match Db::open(DbConfig::read_only(&*self.path)).await {
            Ok(db) => {
                tracing::info!(path = %self.path.display(), "opened the fetch archive");
                *held = Some(db.clone());
                Some(db)
            }
            Err(error) => {
                tracing::debug!(path = %self.path.display(), "no fetch archive yet: {error}");
                None
            }
        }
    }

    pub async fn legacy_fetched_at(&self, date: ApodDate) -> Option<i64> {
        let db = self.db().await?;
        let found: Result<Option<Option<i64>>, _> = sqlx::query_scalar(
            "SELECT fetched_at FROM fetches WHERE date_id = ?1 AND source = 'legacy'",
        )
        .bind(date.days() as i64)
        .fetch_optional(db.reader())
        .await;

        match found {
            Ok(row) => row.flatten(),
            Err(error) => {
                tracing::warn!(%date, "reading the fetch date: {error}");
                None
            }
        }
    }

    pub async fn modern_missing(&self, date: ApodDate) -> bool {
        let Some(db) = self.db().await else {
            return false;
        };

        let found: Result<Option<i64>, _> = sqlx::query_scalar(
            "SELECT http_status FROM fetches WHERE date_id = ?1 AND source = 'modern'",
        )
        .bind(date.days() as i64)
        .fetch_optional(db.reader())
        .await;

        match found {
            Ok(status) => status == Some(404),
            Err(error) => {
                tracing::warn!(%date, "reading the modern fetch status: {error}");
                false
            }
        }
    }

    pub async fn coverage(&self, index: &ApodReader) -> Option<Coverage> {
        let db = self.db().await?;

        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT date_id, http_status FROM fetches
             WHERE source = 'modern' AND http_status IN (200, 404)",
        )
        .fetch_all(db.reader())
        .await
        .inspect_err(|error| tracing::warn!("reading modern coverage: {error}"))
        .ok()?;

        let by_year = index
            .entries_by_year()
            .await
            .inspect_err(|error| tracing::warn!("counting entries by year: {error}"))
            .ok()?;

        let mut measured: BTreeMap<i32, (i64, i64)> = BTreeMap::new();
        let mut absent_dates: Vec<ApodDate> = Vec::new();
        for (days, status) in rows {
            let date = ApodDate::from_days(days as i32);
            let year: i32 = date.format("%Y").parse().unwrap_or_default();
            let slot = measured.entry(year).or_default();
            match status {
                200 => slot.0 += 1,
                _ => {
                    slot.1 += 1;
                    absent_dates.push(date);
                }
            }
        }
        absent_dates.sort_unstable();
        absent_dates.truncate(ABSENT_LISTED);

        let mut total = Split::default();
        let mut entries = 0;
        for (year, count) in by_year {
            let (carried, absent) = measured.get(&year).copied().unwrap_or_default();

            entries += count;
            total.carried += carried;
            total.absent += absent;
            total.unchecked += (count - carried - absent).max(0);
        }

        Some(Coverage {
            entries,
            split: total,
            absent_dates,
        })
    }
}
