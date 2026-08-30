use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

const DEFAULT_MAX_AGE_SECS: u64 = 36 * 3600;

#[derive(Debug, Clone)]
pub struct Config {
    pub index_db: PathBuf,
    pub bot_db: PathBuf,
    pub thumb_dir: PathBuf,
    pub public_url: String,
    pub announce: Announce,
    pub search_page: usize,
    pub page_life: Duration,
    pub owner_ids: HashSet<u64>,
}

#[derive(Debug, Clone)]
pub struct Announce {
    pub enabled: bool,
    pub poll: Duration,
    pub max_age: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir: PathBuf = env_or("APOD_DATA_DIR", "/data".into())?;

        Ok(Self {
            index_db: env_or("APOD_DB", data_dir.join("apod.db"))?,
            bot_db: env_or("APOD_BOT_DB", data_dir.join("bot.db"))?,
            thumb_dir: env_or("APOD_THUMB_DIR", data_dir.join("thumbs"))?,
            public_url: env_or(
                "APOD_PUBLIC_URL",
                "https://apod.lemon.industries".to_owned(),
            )
            .map(|url: String| url.trim_end_matches('/').to_owned())?,

            announce: Announce {
                enabled: env_or("APOD_BOT_ANNOUNCE_ENABLED", true)?,
                poll: Duration::from_secs(env_or("APOD_BOT_POLL_SECS", 60)?),
                max_age: Duration::from_secs(env_or(
                    "APOD_BOT_MAX_AGE_SECS",
                    DEFAULT_MAX_AGE_SECS,
                )?),
            },

            search_page: env_or("APOD_BOT_SEARCH_PAGE", 5)?,
            page_life: Duration::from_secs(env_or("APOD_BOT_PAGE_LIFE_SECS", 300)?),
            owner_ids: ids("APOD_BOT_OWNER_IDS")?,
        })
        .and_then(Self::validated)
    }

    fn validated(self) -> Result<Self> {
        anyhow::ensure!(
            self.announce.poll >= Duration::from_secs(5),
            "APOD_BOT_POLL_SECS must be at least 5: the archive is a local file, but a tighter \
             loop than this buys nothing and only spins"
        );
        anyhow::ensure!(
            (1..=10).contains(&self.search_page),
            "APOD_BOT_SEARCH_PAGE must be between 1 and 10: more than ten hits do not fit one \
             embed"
        );
        Ok(self)
    }

    pub fn thumb_file(&self, path: &str) -> PathBuf {
        self.thumb_dir.join(path)
    }

    pub fn entry_url(&self, date: apod_core::ApodDate) -> String {
        format!("{}/{date}", self.public_url)
    }
}

fn ids(key: &str) -> Result<HashSet<u64>> {
    let Ok(raw) = std::env::var(key) else {
        return Ok(HashSet::new());
    };

    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u64>()
                .with_context(|| format!("{key} holds '{part}', which is not a Discord id"))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Config {
        Config {
            index_db: "apod.db".into(),
            bot_db: "bot.db".into(),
            thumb_dir: "thumbs".into(),
            public_url: "https://apod.example".to_owned(),
            announce: Announce {
                enabled: true,
                poll: Duration::from_secs(60),
                max_age: Duration::from_secs(DEFAULT_MAX_AGE_SECS),
            },
            search_page: 5,
            page_life: Duration::from_secs(300),
            owner_ids: HashSet::new(),
        }
    }

    #[test]
    fn a_poll_tighter_than_the_floor_is_refused_rather_than_quietly_raised() {
        let mut tight = base();
        tight.announce.poll = Duration::from_secs(1);
        assert!(tight.validated().is_err());
    }

    #[test]
    fn a_page_that_cannot_fit_in_one_embed_is_refused() {
        let mut wide = base();
        wide.search_page = 40;
        assert!(wide.validated().is_err());

        let mut empty = base();
        empty.search_page = 0;
        assert!(empty.validated().is_err());
    }

    #[test]
    fn the_thumbnail_root_and_the_public_url_both_come_from_the_shared_data_directory() {
        let cfg = base();
        assert_eq!(
            cfg.thumb_file("2026/08/2026-08-30.webp"),
            PathBuf::from("thumbs/2026/08/2026-08-30.webp")
        );
        assert_eq!(
            cfg.entry_url("2026-08-30".parse().unwrap()),
            "https://apod.example/2026-08-30"
        );
    }
}
