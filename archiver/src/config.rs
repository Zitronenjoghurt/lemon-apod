use anyhow::{Context, Result};
use chrono_tz::Tz;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub html_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub archive_db: PathBuf,
    pub index_db: PathBuf,

    pub source_base_url: String,
    pub user_agent: String,
    pub fetch_timeout: Duration,
    pub fetch_max_retries: u32,
    pub fetch_min_bytes: usize,

    pub backfill: Backfill,
    pub daily: Daily,
    pub recheck_per_day: u32,
    pub thumbs: Thumbs,
}

#[derive(Debug, Clone)]
pub struct Backfill {
    pub enabled: bool,
    pub delay_min: Duration,
    pub delay_max: Duration,
}

#[derive(Debug, Clone)]
pub struct Daily {
    pub enabled: bool,
    pub timezone: Tz,
    pub start_hour: u32,
    pub start_minute: u32,
    pub interval: Duration,
    pub window: Duration,
}

#[derive(Debug, Clone)]
pub struct Thumbs {
    pub enabled: bool,
    pub max_width: u32,
    pub quality: f32,
    pub delay_min: Duration,
    pub delay_max: Duration,
    pub youtube_templates: Vec<String>,
    pub vimeo_oembed_url: String,
    pub video_max_bytes: u64,
    pub video_timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = env_or("APOD_DATA_DIR", "/data".into())?;

        Ok(Self {
            html_dir: env_or("APOD_HTML_DIR", data_dir.join("html"))?,
            thumb_dir: env_or("APOD_THUMB_DIR", data_dir.join("thumbs"))?,
            archive_db: env_or("APOD_ARCHIVE_DB", data_dir.join("archive.db"))?,
            index_db: env_or("APOD_DB", data_dir.join("apod.db"))?,

            source_base_url: env_or(
                "APOD_SOURCE_BASE_URL",
                "https://apod.nasa.gov/apod".to_owned(),
            )?,
            user_agent: env_or(
                "APOD_USER_AGENT",
                format!(
                    "lemon-apod/{} (+https://apod.lemon.industries)",
                    env!("CARGO_PKG_VERSION")
                ),
            )?,
            fetch_timeout: secs("APOD_FETCH_TIMEOUT_SECS", 30)?,
            fetch_max_retries: env_or("APOD_FETCH_MAX_RETRIES", 3)?,
            fetch_min_bytes: env_or("APOD_FETCH_MIN_BYTES", 512)?,

            backfill: Backfill {
                enabled: env_or("APOD_BACKFILL_ENABLED", true)?,
                delay_min: secs("APOD_BACKFILL_DELAY_MIN_SECS", 10)?,
                delay_max: secs("APOD_BACKFILL_DELAY_MAX_SECS", 30)?,
            },

            daily: Daily {
                enabled: env_or("APOD_DAILY_POLL_ENABLED", true)?,
                timezone: env_or("APOD_DAILY_POLL_TZ", Tz::America__New_York)?,
                start_hour: 0,
                start_minute: 0,
                interval: secs("APOD_DAILY_POLL_INTERVAL_SECS", 60)?,
                window: Duration::from_secs(
                    u64::from(env_or::<u32>("APOD_DAILY_POLL_WINDOW_HOURS", 12)?) * 3600,
                ),
            },

            recheck_per_day: env_or("APOD_RECHECK_PER_DAY", 0)?,

            thumbs: Thumbs {
                enabled: env_or("APOD_THUMBS_ENABLED", true)?,
                max_width: env_or("APOD_THUMB_MAX_WIDTH", 480)?,
                quality: env_or("APOD_THUMB_QUALITY", 80.0)?,
                delay_min: secs("APOD_THUMB_DELAY_MIN_SECS", 5)?,
                delay_max: secs("APOD_THUMB_DELAY_MAX_SECS", 15)?,
                youtube_templates: comma_separated(
                    "APOD_YOUTUBE_THUMB_TEMPLATES",
                    "https://i.ytimg.com/vi/{id}/maxresdefault.jpg,\
                     https://i.ytimg.com/vi/{id}/mqdefault.jpg,\
                     https://i.ytimg.com/vi/{id}/hqdefault.jpg",
                )?,
                vimeo_oembed_url: env_or(
                    "APOD_VIMEO_OEMBED_URL",
                    "https://vimeo.com/api/oembed.json".to_owned(),
                )?,
                video_max_bytes: env_or("APOD_VIDEO_MAX_MB", 64u64)? * 1_048_576,
                video_timeout: secs("APOD_VIDEO_TIMEOUT_SECS", 300)?,
            },
        })
        .and_then(Self::validated)
    }

    fn validated(self) -> Result<Self> {
        anyhow::ensure!(
            self.backfill.delay_min <= self.backfill.delay_max,
            "APOD_BACKFILL_DELAY_MIN_SECS must not exceed APOD_BACKFILL_DELAY_MAX_SECS"
        );
        anyhow::ensure!(
            self.thumbs.delay_min <= self.thumbs.delay_max,
            "APOD_THUMB_DELAY_MIN_SECS must not exceed APOD_THUMB_DELAY_MAX_SECS"
        );
        anyhow::ensure!(
            self.thumbs.max_width > 0,
            "APOD_THUMB_MAX_WIDTH must be greater than zero"
        );
        anyhow::ensure!(
            !self.daily.interval.is_zero(),
            "APOD_DAILY_POLL_INTERVAL_SECS must be greater than zero"
        );
        Ok(self)
    }

    pub fn page_url(&self, date: apod_core::ApodDate) -> String {
        format!(
            "{}/ap{}.html",
            self.source_base_url.trim_end_matches('/'),
            date.format("%y%m%d")
        )
    }

    pub fn html_path(&self, date: apod_core::ApodDate) -> PathBuf {
        self.html_dir.join(date.html_path())
    }

    pub fn thumb_path(&self, date: apod_core::ApodDate) -> PathBuf {
        self.thumb_dir.join(date.thumb_path())
    }
}

fn env_or<T>(key: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(raw) if raw.trim().is_empty() => Ok(default),
        Ok(raw) => T::from_str(raw.trim())
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| format!("{key}='{raw}' could not be parsed")),
    }
}

fn secs(key: &str, default: u64) -> Result<Duration> {
    Ok(Duration::from_secs(env_or(key, default)?))
}

fn comma_separated(key: &str, default: &str) -> Result<Vec<String>> {
    let raw: String = env_or(key, default.to_owned())?;
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();

    anyhow::ensure!(!values.is_empty(), "{key} listed no values");
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::ApodDate;

    fn config() -> Config {
        unsafe {
            std::env::set_var("APOD_DATA_DIR", "/tmp/apod-test");
        }
        Config::from_env().unwrap()
    }

    #[test]
    fn derives_paths_from_the_data_directory() {
        let cfg = config();
        assert_eq!(cfg.html_dir, PathBuf::from("/tmp/apod-test/html"));
        assert_eq!(cfg.index_db, PathBuf::from("/tmp/apod-test/apod.db"));
        assert_eq!(
            cfg.html_path(ApodDate::from_ymd(2024, 3, 5).unwrap()),
            PathBuf::from("/tmp/apod-test/html/2024/03/2024-03-05.html")
        );
    }

    #[test]
    fn builds_source_urls() {
        let cfg = config();
        assert_eq!(
            cfg.page_url(ApodDate::from_ymd(2024, 3, 5).unwrap()),
            "https://apod.nasa.gov/apod/ap240305.html"
        );
    }

    #[test]
    fn defaults_the_poll_window_to_eastern_time() {
        assert_eq!(config().daily.timezone, Tz::America__New_York);
    }
}
