use anyhow::{Context, Result};
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Config {
    pub index_db: PathBuf,
    pub thumb_dir: PathBuf,
    pub static_dir: PathBuf,

    pub bind: IpAddr,
    pub port: u16,
    pub public_url: String,

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
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = env_or("APOD_DATA_DIR", "/data".into())?;

        Ok(Self {
            index_db: env_or("APOD_DB", data_dir.join("apod.db"))?,
            thumb_dir: env_or("APOD_THUMB_DIR", data_dir.join("thumbs"))?,
            static_dir: env_or("APOD_STATIC_DIR", "./static".into())?,

            bind: env_or("APOD_BIND", IpAddr::from([0, 0, 0, 0]))?,
            port: env_or("APOD_PORT", 51995)?,
            public_url: env_or(
                "APOD_PUBLIC_URL",
                "https://apod.lemon.industries".to_owned(),
            )
            .map(|url: String| url.trim_end_matches('/').to_owned())?,

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
        })
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
