use anyhow::{Context, Result};
use chrono_tz::Tz;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

const LEGACY_ARCHIVE_URL: Option<&str> = Some(
    "https://github.com/Zitronenjoghurt/apod-legacy-html/raw/refs/heads/main/legacy-html.tar.zst",
);

#[derive(Debug, Clone)]
pub struct Config {
    pub html_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub media_dir: PathBuf,
    pub archive_db: PathBuf,
    pub index_db: PathBuf,
    pub sky_db: PathBuf,
    pub notify_db: PathBuf,
    pub votes_db: PathBuf,
    pub baseline_dir: PathBuf,

    pub source_base_url: String,
    pub legacy_archive_url: Option<String>,
    pub user_agent: String,
    pub fetch_timeout: Duration,
    pub fetch_max_retries: u32,
    pub fetch_min_bytes: usize,

    pub backfill: Backfill,
    pub daily: Daily,
    pub recheck_per_day: u32,
    pub media: MediaArchive,
    pub thumbs: Thumbs,
    pub sky: Sky,
    pub notify: Notify,
}

#[derive(Debug, Clone)]
pub struct Notify {
    pub enabled: bool,
    pub base_url: String,
    /// The same value the server declares in its own `auth-tokens`. Both sides are configured
    /// from one definition rather than one being generated and copied into the other.
    pub token: Option<String>,
    pub public_url: String,
    pub interval: Duration,

    pub apod_topic: Option<String>,
    pub aurora_topic: Option<String>,
    pub space_weather_topic: Option<String>,
    pub sky_topic: Option<String>,

    /// How stale an entry may be and still be worth announcing. Stops a deployment that was down
    /// over a weekend from opening with a picture nobody is looking at any more.
    pub apod_max_age: Duration,
    /// How far ahead a shower peak or a supermoon is announced.
    pub sky_lead: Duration,
    /// Eclipses get longer notice than the rest: they are rare, and worth travelling for.
    pub eclipse_lead: Duration,
    /// Below this Kp, a storm is not worth a phone buzzing at three in the morning.
    pub aurora_min_kp: f64,
}

impl Notify {
    pub fn topics(&self) -> Vec<&str> {
        [
            self.apod_topic.as_deref(),
            self.aurora_topic.as_deref(),
            self.space_weather_topic.as_deref(),
            self.sky_topic.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Sky {
    pub enabled: bool,
    pub launches_enabled: bool,
    pub weather_enabled: bool,
    pub launches_url: String,
    pub launch_page_url: String,
    pub swpc_base_url: String,
    pub launch_limit: u32,
    pub interval: Duration,
}

#[derive(Debug, Clone)]
pub struct Backfill {
    pub enabled: bool,
    pub delay_min: Duration,
    pub delay_max: Duration,
}

/// When APOD publishes, and how hard to look for it. The first three describe the same fact the
/// API serves to the front page from `APOD_PUBLISH_*`, so both services read those keys and a
/// deployment that moves the schedule moves both at once.
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
pub struct MediaArchive {
    pub enabled: bool,
    pub max_bytes: u64,
    pub max_attempts: u32,
    pub timeout: Duration,
    pub delay_min: Duration,
    pub delay_max: Duration,
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
    pub image_max_bytes: u64,
    pub video_max_bytes: u64,
    pub video_timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = env_or("APOD_DATA_DIR", "/data".into())?;

        Ok(Self {
            html_dir: env_or("APOD_HTML_DIR", data_dir.join("html"))?,
            thumb_dir: env_or("APOD_THUMB_DIR", data_dir.join("thumbs"))?,
            media_dir: env_or("APOD_MEDIA_DIR", data_dir.join("media"))?,
            archive_db: env_or("APOD_ARCHIVE_DB", data_dir.join("archive.db"))?,
            index_db: env_or("APOD_DB", data_dir.join("apod.db"))?,
            sky_db: env_or("APOD_SKY_DB", data_dir.join("sky.db"))?,
            notify_db: env_or("APOD_NOTIFY_DB", data_dir.join("notify.db"))?,
            votes_db: env_or("APOD_VOTES_DB", data_dir.join("votes.db"))?,
            baseline_dir: env_or("APOD_BASELINE_DIR", "./baseline".into())?,

            source_base_url: env_or(
                "APOD_SOURCE_BASE_URL",
                "https://apod.nasa.gov/apod".to_owned(),
            )?,
            legacy_archive_url: optional("APOD_LEGACY_ARCHIVE_URL")
                .or_else(|| LEGACY_ARCHIVE_URL.map(str::to_owned)),
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
                timezone: env_or(
                    "APOD_PUBLISH_TZ",
                    env_or("APOD_DAILY_POLL_TZ", Tz::America__New_York)?,
                )?,
                start_hour: env_or("APOD_PUBLISH_HOUR", 0)?,
                start_minute: env_or("APOD_PUBLISH_MINUTE", 0)?,
                interval: secs("APOD_DAILY_POLL_INTERVAL_SECS", 60)?,
                window: Duration::from_secs(
                    u64::from(env_or::<u32>("APOD_DAILY_POLL_WINDOW_HOURS", 12)?) * 3600,
                ),
            },

            recheck_per_day: env_or("APOD_RECHECK_PER_DAY", 0)?,

            media: MediaArchive {
                enabled: env_or("APOD_MEDIA_ENABLED", true)?,
                max_bytes: env_or("APOD_MEDIA_MAX_BYTES", 512 * 1_048_576u64)?,
                max_attempts: env_or("APOD_MEDIA_MAX_ATTEMPTS", 8)?,
                timeout: secs("APOD_MEDIA_TIMEOUT_SECS", 600)?,
                delay_min: millis("APOD_MEDIA_DELAY_MIN_MS", 10_000)?,
                delay_max: millis("APOD_MEDIA_DELAY_MAX_MS", 20_000)?,
            },

            sky: Sky {
                enabled: env_or("APOD_SKY_ENABLED", true)?,
                launches_enabled: env_or("APOD_SKY_LAUNCHES_ENABLED", true)?,
                weather_enabled: env_or("APOD_SKY_WEATHER_ENABLED", true)?,
                launches_url: env_or(
                    "APOD_SKY_LAUNCHES_URL",
                    "https://ll.thespacedevs.com/2.3.0/launches/upcoming/".to_owned(),
                )?,
                launch_page_url: env_or(
                    "APOD_SKY_LAUNCH_PAGE_URL",
                    "https://spacelaunchnow.me/launch/{slug}/".to_owned(),
                )?,
                swpc_base_url: env_or(
                    "APOD_SWPC_BASE_URL",
                    "https://services.swpc.noaa.gov/products".to_owned(),
                )
                .map(|url: String| url.trim_end_matches('/').to_owned())?,
                launch_limit: env_or("APOD_SKY_LAUNCH_LIMIT", 20)?,
                interval: secs("APOD_SKY_INTERVAL_SECS", 1_800)?,
            },

            notify: Notify {
                enabled: env_or("APOD_NOTIFY_ENABLED", false)?,
                base_url: env_or("APOD_NTFY_URL", "https://ntfy.sh".to_owned())
                    .map(|url: String| url.trim_end_matches('/').to_owned())?,
                token: optional("APOD_NTFY_TOKEN"),
                public_url: env_or(
                    "APOD_PUBLIC_URL",
                    "https://apod.lemon.industries".to_owned(),
                )
                .map(|url: String| url.trim_end_matches('/').to_owned())?,
                interval: secs("APOD_NOTIFY_INTERVAL_SECS", 300)?,

                apod_topic: optional("APOD_NTFY_TOPIC_APOD"),
                aurora_topic: optional("APOD_NTFY_TOPIC_AURORA"),
                space_weather_topic: optional("APOD_NTFY_TOPIC_SPACE_WEATHER"),
                sky_topic: optional("APOD_NTFY_TOPIC_SKY"),

                apod_max_age: Duration::from_secs(
                    u64::from(env_or::<u32>("APOD_NOTIFY_APOD_MAX_AGE_HOURS", 36)?) * 3600,
                ),
                sky_lead: Duration::from_secs(
                    u64::from(env_or::<u32>("APOD_NOTIFY_SKY_LEAD_HOURS", 24)?) * 3600,
                ),
                eclipse_lead: Duration::from_secs(
                    u64::from(env_or::<u32>("APOD_NOTIFY_ECLIPSE_LEAD_HOURS", 72)?) * 3600,
                ),
                aurora_min_kp: env_or("APOD_NOTIFY_AURORA_MIN_KP", 5.0)?,
            },

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
                image_max_bytes: env_or("APOD_IMAGE_MAX_MB", 64u64)? * 1_048_576,
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
            self.media.delay_min <= self.media.delay_max,
            "APOD_MEDIA_DELAY_MIN_MS must not exceed APOD_MEDIA_DELAY_MAX_MS"
        );
        anyhow::ensure!(
            self.media.max_bytes > 0,
            "APOD_MEDIA_MAX_BYTES must be greater than zero"
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
        anyhow::ensure!(
            self.daily.start_hour < 24,
            "APOD_PUBLISH_HOUR must be an hour of the day, 0 to 23"
        );
        anyhow::ensure!(
            self.daily.start_minute < 60,
            "APOD_PUBLISH_MINUTE must be a minute of the hour, 0 to 59"
        );
        anyhow::ensure!(
            !self.notify.enabled || !self.notify.topics().is_empty(),
            "APOD_NOTIFY_ENABLED is on but no APOD_NTFY_TOPIC_* was set, so nothing could be sent"
        );
        anyhow::ensure!(
            !self.notify.interval.is_zero(),
            "APOD_NOTIFY_INTERVAL_SECS must be greater than zero"
        );
        anyhow::ensure!(
            self.sky.interval >= Duration::from_secs(60),
            "APOD_SKY_INTERVAL_SECS must be at least 60; NOAA's feeds do not move faster than \
             that and there is no reason to ask them to"
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

    pub fn media_path(&self, stored_path: &str) -> PathBuf {
        self.media_dir.join(stored_path)
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

fn millis(key: &str, default: u64) -> Result<Duration> {
    Ok(Duration::from_millis(env_or(key, default)?))
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
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

    #[test]
    fn defaults_the_publication_window_to_midnight() {
        let daily = config().daily;
        assert_eq!((daily.start_hour, daily.start_minute), (0, 0));
    }

    #[test]
    fn refuses_a_publication_hour_that_is_not_an_hour() {
        let mut cfg = config();
        cfg.daily.start_hour = 24;

        let refused = cfg.validated().unwrap_err().to_string();
        assert!(refused.contains("APOD_PUBLISH_HOUR"), "{refused}");
    }
}
