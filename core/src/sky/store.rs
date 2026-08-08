use crate::db::{Db, DbConfig, DbResult};
use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use sqlx::Row;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use std::path::Path;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations-sky");

pub const LAUNCHES: &str = "launches";
pub const SPACE_WEATHER: &str = "space_weather";

#[derive(Debug, Clone, Serialize)]
pub struct Launch {
    pub id: String,
    pub name: String,
    pub provider: Option<String>,
    pub vehicle: Option<String>,
    pub pad: Option<String>,
    pub mission: Option<String>,
    pub orbit: Option<String>,
    pub status: Option<String>,
    pub net: DateTime<Utc>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub precision: Option<String>,
    pub image_url: Option<String>,
    pub info_url: Option<String>,
}

impl Launch {
    pub fn time_is_firm(&self) -> bool {
        matches!(
            self.precision.as_deref(),
            Some("SEC" | "MIN" | "HR" | "Second" | "Minute" | "Hour")
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SpaceWeather {
    pub kp: f64,
    pub observed_at: DateTime<Utc>,
}

impl SpaceWeather {
    pub fn is_storm(&self) -> bool {
        self.kp >= 5.0
    }

    pub fn label(&self) -> &'static str {
        match self.kp {
            kp if kp >= 8.0 => "Severe geomagnetic storm",
            kp if kp >= 7.0 => "Strong geomagnetic storm",
            kp if kp >= 6.0 => "Moderate geomagnetic storm",
            kp if kp >= 5.0 => "Minor geomagnetic storm",
            kp if kp >= 4.0 => "Unsettled",
            _ => "Quiet",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedState {
    pub name: String,
    pub fetched_at: Option<DateTime<Utc>>,
    pub succeeded: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SkyReader {
    db: Db,
}

impl SkyReader {
    pub async fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        Ok(Self {
            db: Db::open(DbConfig::read_only(path.as_ref())).await?,
        })
    }

    pub async fn upcoming_launches(
        &self,
        from: DateTime<Utc>,
        limit: i64,
    ) -> DbResult<Vec<Launch>> {
        let rows = sqlx::query(
            "SELECT id, name, provider, vehicle, pad, mission, orbit, status, net,
                    window_start, window_end, precision, image_url, info_url
             FROM launches
             WHERE net >= ?1
             ORDER BY net ASC
             LIMIT ?2",
        )
        .bind(from.timestamp())
        .bind(limit)
        .fetch_all(self.db.reader())
        .await?;

        Ok(rows.iter().map(launch_from_row).collect())
    }

    pub async fn space_weather(&self) -> DbResult<Option<SpaceWeather>> {
        let row: Option<(f64, i64)> =
            sqlx::query_as("SELECT kp, observed_at FROM space_weather WHERE id = 1")
                .fetch_optional(self.db.reader())
                .await?;

        Ok(row.map(|(kp, observed_at)| SpaceWeather {
            kp,
            observed_at: timestamp(observed_at),
        }))
    }

    pub async fn feeds(&self) -> DbResult<Vec<FeedState>> {
        let rows: Vec<(String, Option<i64>, i64, Option<String>)> =
            sqlx::query_as("SELECT name, fetched_at, succeeded, error FROM feeds ORDER BY name")
                .fetch_all(self.db.reader())
                .await?;

        Ok(rows
            .into_iter()
            .map(|(name, fetched_at, succeeded, error)| FeedState {
                name,
                fetched_at: fetched_at.map(timestamp),
                succeeded: succeeded != 0,
                error,
            })
            .collect())
    }

    pub async fn close(&self) {
        self.db.close().await;
    }
}

#[derive(Debug, Clone)]
pub struct SkyWriter {
    db: Db,
}

impl SkyWriter {
    pub async fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let db = Db::open(DbConfig::read_write(path.as_ref())).await?;
        db.migrate(&MIGRATIONS).await?;
        Ok(Self { db })
    }

    pub fn reader(&self) -> SkyReader {
        SkyReader {
            db: self.db.clone(),
        }
    }

    pub async fn replace_launches(
        &self,
        launches: &[Launch],
        keep_from: DateTime<Utc>,
    ) -> DbResult<u64> {
        let writer = self.db.writer()?;
        let mut transaction = writer.begin().await?;
        let now = Utc::now().timestamp();

        sqlx::query("DELETE FROM launches")
            .execute(&mut *transaction)
            .await?;

        let mut written = 0;
        for launch in launches.iter().filter(|launch| launch.net >= keep_from) {
            sqlx::query(
                "INSERT INTO launches (id, name, provider, vehicle, pad, mission, orbit, status,
                                       net, window_start, window_end, precision, image_url,
                                       info_url, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(&launch.id)
            .bind(&launch.name)
            .bind(&launch.provider)
            .bind(&launch.vehicle)
            .bind(&launch.pad)
            .bind(&launch.mission)
            .bind(&launch.orbit)
            .bind(&launch.status)
            .bind(launch.net.timestamp())
            .bind(launch.window_start.map(|at| at.timestamp()))
            .bind(launch.window_end.map(|at| at.timestamp()))
            .bind(&launch.precision)
            .bind(&launch.image_url)
            .bind(&launch.info_url)
            .bind(now)
            .execute(&mut *transaction)
            .await?;

            written += 1;
        }

        transaction.commit().await?;
        Ok(written)
    }

    pub async fn set_space_weather(&self, weather: SpaceWeather) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO space_weather (id, kp, observed_at, updated_at)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
               kp = excluded.kp,
               observed_at = excluded.observed_at,
               updated_at = excluded.updated_at",
        )
        .bind(weather.kp)
        .bind(weather.observed_at.timestamp())
        .bind(Utc::now().timestamp())
        .execute(self.db.writer()?)
        .await?;

        Ok(())
    }

    pub async fn record_feed(&self, name: &str, error: Option<&str>) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO feeds (name, fetched_at, succeeded, error)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
               fetched_at = excluded.fetched_at,
               succeeded = excluded.succeeded,
               error = excluded.error",
        )
        .bind(name)
        .bind(Utc::now().timestamp())
        .bind(i64::from(error.is_none()))
        .bind(error)
        .execute(self.db.writer()?)
        .await?;

        Ok(())
    }

    pub async fn close(&self) {
        self.db.close().await;
    }
}

fn launch_from_row(row: &SqliteRow) -> Launch {
    Launch {
        id: row.get("id"),
        name: row.get("name"),
        provider: row.get("provider"),
        vehicle: row.get("vehicle"),
        pad: row.get("pad"),
        mission: row.get("mission"),
        orbit: row.get("orbit"),
        status: row.get("status"),
        net: timestamp(row.get("net")),
        window_start: row.get::<Option<i64>, _>("window_start").map(timestamp),
        window_end: row.get::<Option<i64>, _>("window_end").map(timestamp),
        precision: row.get("precision"),
        image_url: row.get("image_url"),
        info_url: row.get("info_url"),
    }
}

fn timestamp(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .unwrap_or(DateTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_path() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apod-sky-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("sky.db")
    }

    fn launch(id: &str, minutes_out: i64) -> Launch {
        Launch {
            id: id.to_owned(),
            name: format!("Falcon 9 | {id}"),
            provider: Some("SpaceX".to_owned()),
            vehicle: Some("Falcon 9 Block 5".to_owned()),
            pad: Some("SLC-40, Cape Canaveral".to_owned()),
            mission: Some("A batch of satellites".to_owned()),
            orbit: Some("Low Earth Orbit".to_owned()),
            status: Some("Go for Launch".to_owned()),
            net: Utc::now() + TimeDelta::minutes(minutes_out),
            window_start: None,
            window_end: None,
            precision: Some("SEC".to_owned()),
            image_url: None,
            info_url: None,
        }
    }

    #[tokio::test]
    async fn a_fresh_database_migrates_and_reads_back_empty() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();
        let reader = writer.reader();

        assert!(
            reader
                .upcoming_launches(Utc::now(), 10)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(reader.space_weather().await.unwrap().is_none());
        assert!(reader.feeds().await.unwrap().is_empty());

        writer.close().await;
    }

    #[tokio::test]
    async fn launches_come_back_soonest_first() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();

        let written = writer
            .replace_launches(
                &[launch("c", 300), launch("a", 60), launch("b", 120)],
                Utc::now(),
            )
            .await
            .unwrap();
        assert_eq!(written, 3);

        let found = writer
            .reader()
            .upcoming_launches(Utc::now(), 10)
            .await
            .unwrap();

        let ids: Vec<&str> = found.iter().map(|launch| launch.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);

        writer.close().await;
    }

    #[tokio::test]
    async fn a_replacement_drops_launches_that_have_gone_away() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();

        writer
            .replace_launches(&[launch("a", 60), launch("b", 120)], Utc::now())
            .await
            .unwrap();
        writer
            .replace_launches(&[launch("b", 120)], Utc::now())
            .await
            .unwrap();

        let found = writer
            .reader()
            .upcoming_launches(Utc::now(), 10)
            .await
            .unwrap();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "b");

        writer.close().await;
    }

    #[tokio::test]
    async fn anything_already_flown_is_never_written() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();

        let written = writer
            .replace_launches(&[launch("past", -600), launch("future", 600)], Utc::now())
            .await
            .unwrap();

        assert_eq!(written, 1);
        let found = writer
            .reader()
            .upcoming_launches(Utc::now() - TimeDelta::days(30), 10)
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "future");

        writer.close().await;
    }

    #[tokio::test]
    async fn every_field_of_a_launch_survives_the_round_trip() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();
        let original = launch("full", 90);

        writer
            .replace_launches(std::slice::from_ref(&original), Utc::now())
            .await
            .unwrap();

        let found = &writer
            .reader()
            .upcoming_launches(Utc::now(), 10)
            .await
            .unwrap()[0];

        assert_eq!(found.name, original.name);
        assert_eq!(found.provider, original.provider);
        assert_eq!(found.vehicle, original.vehicle);
        assert_eq!(found.pad, original.pad);
        assert_eq!(found.mission, original.mission);
        assert_eq!(found.orbit, original.orbit);
        assert_eq!(found.status, original.status);
        assert_eq!(found.precision, original.precision);
        assert_eq!(found.net.timestamp(), original.net.timestamp());

        writer.close().await;
    }

    #[tokio::test]
    async fn space_weather_holds_only_the_latest_reading() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();
        let observed = Utc::now();

        writer
            .set_space_weather(SpaceWeather {
                kp: 2.0,
                observed_at: observed,
            })
            .await
            .unwrap();
        writer
            .set_space_weather(SpaceWeather {
                kp: 6.33,
                observed_at: observed + TimeDelta::hours(3),
            })
            .await
            .unwrap();

        let found = writer.reader().space_weather().await.unwrap().unwrap();
        assert!((found.kp - 6.33).abs() < 1e-9);
        assert!(found.is_storm());
        assert_eq!(found.label(), "Moderate geomagnetic storm");

        writer.close().await;
    }

    #[tokio::test]
    async fn the_kp_scale_reads_the_way_noaa_words_it() {
        let at = Utc::now();
        for (kp, expected, storm) in [
            (0.0, "Quiet", false),
            (3.67, "Quiet", false),
            (4.0, "Unsettled", false),
            (5.0, "Minor geomagnetic storm", true),
            (6.0, "Moderate geomagnetic storm", true),
            (7.0, "Strong geomagnetic storm", true),
            (9.0, "Severe geomagnetic storm", true),
        ] {
            let weather = SpaceWeather {
                kp,
                observed_at: at,
            };
            assert_eq!(weather.label(), expected, "kp {kp}");
            assert_eq!(weather.is_storm(), storm, "kp {kp}");
        }
    }

    #[tokio::test]
    async fn a_feed_records_both_its_successes_and_its_failures() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();

        writer.record_feed(LAUNCHES, None).await.unwrap();
        writer
            .record_feed(SPACE_WEATHER, Some("the host went away"))
            .await
            .unwrap();

        let feeds = writer.reader().feeds().await.unwrap();
        assert_eq!(feeds.len(), 2);

        let launches = feeds.iter().find(|feed| feed.name == LAUNCHES).unwrap();
        assert!(launches.succeeded);
        assert!(launches.error.is_none());
        assert!(launches.fetched_at.is_some());

        let weather = feeds
            .iter()
            .find(|feed| feed.name == SPACE_WEATHER)
            .unwrap();
        assert!(!weather.succeeded);
        assert_eq!(weather.error.as_deref(), Some("the host went away"));

        writer.record_feed(SPACE_WEATHER, None).await.unwrap();
        let feeds = writer.reader().feeds().await.unwrap();
        let weather = feeds
            .iter()
            .find(|feed| feed.name == SPACE_WEATHER)
            .unwrap();
        assert!(weather.succeeded);
        assert!(weather.error.is_none());

        writer.close().await;
    }

    #[tokio::test]
    async fn the_limit_is_honoured() {
        let writer = SkyWriter::open(temp_path()).await.unwrap();
        let many: Vec<Launch> = (0..20)
            .map(|index| launch(&format!("l{index:02}"), 60 + index * 10))
            .collect();

        writer.replace_launches(&many, Utc::now()).await.unwrap();

        let found = writer
            .reader()
            .upcoming_launches(Utc::now(), 5)
            .await
            .unwrap();
        assert_eq!(found.len(), 5);
        assert_eq!(found[0].id, "l00");

        writer.close().await;
    }

    #[tokio::test]
    async fn a_firm_time_is_told_apart_from_a_guess() {
        let mut soon = launch("soon", 30);
        assert!(soon.time_is_firm());

        for vague in ["DAY", "MONTH", "YEAR", "Quarter"] {
            soon.precision = Some(vague.to_owned());
            assert!(!soon.time_is_firm(), "{vague} read as firm");
        }

        soon.precision = None;
        assert!(!soon.time_is_firm());
    }

    #[tokio::test]
    async fn a_reader_cannot_write() {
        let path = temp_path();
        SkyWriter::open(&path).await.unwrap().close().await;

        let reader = SkyReader::open(&path).await.unwrap();
        assert!(reader.upcoming_launches(Utc::now(), 5).await.is_ok());
        reader.close().await;
    }
}
