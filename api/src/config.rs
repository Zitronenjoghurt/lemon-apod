use anyhow::{Context, Result};
use chrono::TimeDelta;
use chrono_tz::Tz;
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub index_db: PathBuf,
    pub sky_db: PathBuf,
    pub votes_db: PathBuf,
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

    pub rating: Rating,
    pub contact: Contact,
    pub notify: Notify,
}

#[derive(Clone, Default)]
pub struct Secret(Option<String>);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            Some(_) => "Secret(set)",
            None => "Secret(unset)",
        })
    }
}

impl Secret {
    pub fn bytes(&self) -> Vec<u8> {
        match &self.0 {
            Some(secret) => secret.as_bytes().to_vec(),
            None => {
                tracing::warn!(
                    "APOD_RATING_SECRET is not set, so ballots and cohort linkage will not \
                     survive a restart"
                );
                use rand::RngExt;
                (0..32).map(|_| rand::rng().random()).collect()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rating {
    pub enabled: bool,
    /// How long a ballot is accepted at all. This also sizes the spent-ballot memory, so it is the
    /// hard ceiling: past it a vote is refused and the client is handed another ballot.
    pub ballot_life: Duration,
    /// Past this a ballot still votes, but the vote is noted as having sat a while. A stale ballot
    /// is a signal worth recording rather than a reason to throw a judgment away.
    pub ballot_fresh: Duration,
    /// A vote returned faster than this was not a judgment. Also recorded rather than refused.
    pub min_response: Duration,
    /// The sliding life of a voter's receipt, refreshed on every vote.
    pub cookie_life: Duration,
    /// How long a cohort hash stays on a voter row before it is dropped.
    pub cohort_life: Duration,
    /// The window the vote budgets are measured over, and what fits in it.
    pub budget_window: Duration,
    pub votes_per_window: u64,
    pub cohort_votes_per_window: u64,
    /// How many times one voter may weigh in on any one picture, so no individual can move a
    /// score far on their own.
    pub per_picture: u32,
    /// How many of a voter's most recent ballots count as "in quick succession".
    pub recent: usize,
    /// How long before a pair is worth putting back in front of the same voter.
    pub probe_after: Duration,
    pub fit_every: Duration,
    /// The share of ballots that ask the first question, out of a hundred.
    pub beautiful_share: u32,
    /// Voting gets its own allowance, because a session is one request per vote and the site's
    /// browsing allowance replenishes one request every few seconds after its burst. The real
    /// limit on voting is `votes_per_window`, measured against the log.
    pub vote_limit_period: Duration,
    pub vote_limit_burst: u32,
    pub board_default_limit: usize,
    pub board_max_limit: usize,
    pub secret: Secret,
}

impl Rating {
    pub fn secret(&self) -> Vec<u8> {
        self.secret.bytes()
    }

    pub fn ballot_life_delta(&self) -> TimeDelta {
        delta(self.ballot_life)
    }

    pub fn ballot_fresh_delta(&self) -> TimeDelta {
        delta(self.ballot_fresh)
    }

    pub fn min_response_delta(&self) -> TimeDelta {
        delta(self.min_response)
    }

    fn from_env() -> Result<Self> {
        Ok(Self {
            enabled: env_or("APOD_RATING_ENABLED", true)?,

            ballot_life: secs(env_or("APOD_RATING_BALLOT_LIFE_SECS", 3_600)?),
            ballot_fresh: secs(env_or("APOD_RATING_BALLOT_FRESH_SECS", 300)?),
            min_response: Duration::from_millis(env_or("APOD_RATING_MIN_RESPONSE_MS", 400)?),

            cookie_life: days(env_or("APOD_RATING_COOKIE_DAYS", 90)?),
            cohort_life: days(env_or("APOD_RATING_COHORT_DAYS", 30)?),

            budget_window: secs(env_or("APOD_RATING_BUDGET_WINDOW_SECS", 3_600)?),
            votes_per_window: env_or("APOD_RATING_VOTES_PER_WINDOW", 300)?,
            cohort_votes_per_window: env_or("APOD_RATING_COHORT_VOTES_PER_WINDOW", 1_000)?,

            per_picture: env_or("APOD_RATING_PER_PICTURE", 3)?,
            recent: env_or("APOD_RATING_RECENT", 12)?,
            probe_after: secs(env_or("APOD_RATING_PROBE_AFTER_SECS", 300)?),

            fit_every: secs(env_or("APOD_RATING_FIT_SECS", 300)?),
            beautiful_share: env_or("APOD_RATING_BEAUTIFUL_SHARE", 65)?,

            vote_limit_period: Duration::from_millis(env_or(
                "APOD_RATING_VOTE_LIMIT_PERIOD_MS",
                500,
            )?),
            vote_limit_burst: env_or("APOD_RATING_VOTE_LIMIT_BURST", 60)?,

            board_default_limit: env_or("APOD_RATING_BOARD_DEFAULT_LIMIT", 50)?,
            board_max_limit: env_or("APOD_RATING_BOARD_MAX_LIMIT", 200)?,

            secret: Secret(optional("APOD_RATING_SECRET")),
        })
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.beautiful_share <= 100,
            "APOD_RATING_BEAUTIFUL_SHARE must be a share out of a hundred"
        );
        anyhow::ensure!(
            self.per_picture >= 1,
            "APOD_RATING_PER_PICTURE of nothing would leave no pair anybody may vote on"
        );
        anyhow::ensure!(
            self.ballot_fresh <= self.ballot_life,
            "APOD_RATING_BALLOT_FRESH_SECS cannot outlast APOD_RATING_BALLOT_LIFE_SECS"
        );
        Ok(())
    }
}

fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn days(days: u64) -> Duration {
    Duration::from_secs(days * 86_400)
}

fn delta(duration: Duration) -> TimeDelta {
    TimeDelta::from_std(duration).unwrap_or(TimeDelta::MAX)
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
            votes_db: env_or("APOD_VOTES_DB", data_dir.join("votes.db"))?,
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

            rating: Rating::from_env()?,

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
        self.rating.validate()?;
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
