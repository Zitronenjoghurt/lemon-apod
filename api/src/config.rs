use anyhow::{Context, Result};
use chrono_tz::Tz;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Config {
    pub index_db: PathBuf,
    pub sky_db: PathBuf,
    pub thumb_dir: PathBuf,
    pub static_dir: PathBuf,

    pub bind: IpAddr,
    pub port: u16,
    pub public_url: String,

    pub publish: Publish,

    pub list_default_limit: usize,
    pub list_max_limit: usize,
    pub search_default_limit: usize,
    pub search_max_limit: usize,
    pub search_snippet_tokens: usize,

    pub rate_limit_per_second: u64,
    pub rate_limit_burst: u32,

    pub cache_entry_secs: u64,
    pub cache_latest_secs: u64,
    pub cache_list_secs: u64,
    pub cache_sitemap_secs: u64,
    pub cache_timeline_secs: u64,
    pub cache_status_secs: u64,
    pub cache_sky_secs: u64,
    pub cache_feed_secs: u64,

    pub feed_limit: usize,

    pub sky_launch_limit: i64,

    pub contact: Contact,
    pub notify: Notify,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Contact {
    pub form_key: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Notify {
    pub base_url: Option<String>,
    pub apod_topic: Option<String>,
    pub aurora_topic: Option<String>,
    pub space_weather_topic: Option<String>,
    pub sky_topic: Option<String>,
}

impl Notify {
    fn from_env() -> Self {
        let base_url = optional("APOD_NTFY_URL").map(|url| url.trim_end_matches('/').to_owned());

        match base_url {
            None => Self::default(),
            Some(base_url) => Self {
                base_url: Some(base_url),
                apod_topic: optional("APOD_NTFY_TOPIC_APOD"),
                aurora_topic: optional("APOD_NTFY_TOPIC_AURORA"),
                space_weather_topic: optional("APOD_NTFY_TOPIC_SPACE_WEATHER"),
                sky_topic: optional("APOD_NTFY_TOPIC_SKY"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Publish {
    pub timezone: Tz,
    pub hour: u32,
    pub minute: u32,
}

impl Publish {
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.hour < 24,
            "APOD_PUBLISH_HOUR must be an hour of the day, 0 to 23"
        );
        anyhow::ensure!(
            self.minute < 60,
            "APOD_PUBLISH_MINUTE must be a minute of the hour, 0 to 59"
        );
        Ok(())
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = env_or("APOD_DATA_DIR", "/data".into())?;

        Ok(Self {
            index_db: env_or("APOD_DB", data_dir.join("apod.db"))?,
            sky_db: env_or("APOD_SKY_DB", data_dir.join("sky.db"))?,
            thumb_dir: env_or("APOD_THUMB_DIR", data_dir.join("thumbs"))?,
            static_dir: env_or("APOD_STATIC_DIR", "./static".into())?,

            bind: env_or("APOD_BIND", IpAddr::from([0, 0, 0, 0]))?,
            port: env_or("APOD_PORT", 51995)?,
            public_url: env_or(
                "APOD_PUBLIC_URL",
                "https://apod.lemon.industries".to_owned(),
            )
            .map(|url: String| url.trim_end_matches('/').to_owned())?,

            publish: Publish {
                timezone: env_or("APOD_PUBLISH_TZ", Tz::America__New_York)?,
                hour: env_or("APOD_PUBLISH_HOUR", 0)?,
                minute: env_or("APOD_PUBLISH_MINUTE", 0)?,
            },

            list_default_limit: env_or("APOD_LIST_DEFAULT_LIMIT", 30)?,
            list_max_limit: env_or("APOD_LIST_MAX_LIMIT", 100)?,
            search_default_limit: env_or("APOD_SEARCH_DEFAULT_LIMIT", 30)?,
            search_max_limit: env_or("APOD_SEARCH_MAX_LIMIT", 100)?,
            search_snippet_tokens: env_or("APOD_SEARCH_SNIPPET_TOKENS", 32)?,

            rate_limit_per_second: env_or("APOD_RATE_LIMIT_PER_SECOND", 5)?,
            rate_limit_burst: env_or("APOD_RATE_LIMIT_BURST", 30)?,

            cache_entry_secs: env_or("APOD_CACHE_ENTRY_SECS", 86_400)?,
            cache_latest_secs: env_or("APOD_CACHE_LATEST_SECS", 300)?,
            cache_list_secs: env_or("APOD_CACHE_LIST_SECS", 300)?,
            cache_sitemap_secs: env_or("APOD_CACHE_SITEMAP_SECS", 3_600)?,
            cache_timeline_secs: env_or("APOD_CACHE_TIMELINE_SECS", 3_600)?,
            cache_status_secs: env_or("APOD_CACHE_STATUS_SECS", 60)?,
            cache_sky_secs: env_or("APOD_CACHE_SKY_SECS", 1800)?,
            cache_feed_secs: env_or("APOD_CACHE_FEED_SECS", 3_600)?,

            feed_limit: env_or("APOD_FEED_LIMIT", 25)?,

            sky_launch_limit: env_or("APOD_SKY_LAUNCH_LIMIT", 10)?,

            contact: Contact {
                form_key: optional("APOD_CONTACT_FORM_KEY"),
                email: optional("APOD_CONTACT_EMAIL"),
            },

            notify: Notify::from_env(),
        })
        .and_then(Self::validated)
    }

    fn validated(self) -> Result<Self> {
        self.publish.validate()?;
        Ok(self)
    }
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
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
