use crate::config::Config;
use crate::meta::Shell;
use crate::rating::Rating;
use anyhow::Result;
use apod_core::db::DbConfig;
use apod_core::sky::store::SkyReader;
use apod_core::{ApodReader, Snippet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const DB_READERS: u32 = 8;
const HEALTH_TTL: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<Config>,
    pub store: ApodReader,
    pub sky: Sky,
    pub rating: Option<Arc<Rating>>,
    pub shell: Arc<Shell>,
    pub sitemap: Cached,
    pub atom: Cached,
    pub rss: Cached,
    pub timeline: Cached,
    pub coverage: Cached,
    pub health: Cached,
}

impl ServerState {
    pub async fn new(config: Config) -> Result<Self> {
        let store =
            ApodReader::open(DbConfig::read_only(&config.index_db).with_readers(DB_READERS))
                .await?
                .with_thumb_base("/thumbs/")
                .with_snippet(Snippet::Html);

        let rating = match config.rating.enabled {
            false => None,
            true => match Rating::open(&config.votes_db, config.rating.clone()).await {
                Ok(rating) => Some(Arc::new(rating)),
                Err(error) => {
                    tracing::error!("rating is off: {error:#}");
                    None
                }
            },
        };

        Ok(Self {
            rating,
            shell: Arc::new(Shell::load(&config)?),
            sky: Sky::new(
                config.sky_db.clone(),
                Duration::from_secs(config.cache_sky_secs),
            ),
            sitemap: Cached::new(Duration::from_secs(config.cache_sitemap_secs)),
            atom: Cached::new(Duration::from_secs(config.cache_feed_secs)),
            rss: Cached::new(Duration::from_secs(config.cache_feed_secs)),
            timeline: Cached::new(Duration::from_secs(config.cache_timeline_secs)),
            coverage: Cached::new(Duration::from_secs(config.cache_timeline_secs)),
            health: Cached::new(HEALTH_TTL),
            store,
            config: Arc::new(config),
        })
    }
}

#[derive(Clone)]
pub struct Sky {
    path: Arc<PathBuf>,
    reader: Arc<RwLock<Option<SkyReader>>>,
    pub cached: Cached,
}

impl Sky {
    fn new(path: PathBuf, ttl: Duration) -> Self {
        Self {
            path: Arc::new(path),
            reader: Arc::new(RwLock::new(None)),
            cached: Cached::new(ttl),
        }
    }

    pub async fn reader(&self) -> Option<SkyReader> {
        if let Some(reader) = self.reader.read().await.as_ref() {
            return Some(reader.clone());
        }

        let mut held = self.reader.write().await;
        if let Some(reader) = held.as_ref() {
            return Some(reader.clone());
        }

        match SkyReader::open(&*self.path).await {
            Ok(reader) => {
                tracing::info!(path = %self.path.display(), "opened the sky database");
                *held = Some(reader.clone());
                Some(reader)
            }
            Err(error) => {
                tracing::debug!(path = %self.path.display(), "no sky database yet: {error}");
                None
            }
        }
    }
}

struct Held {
    built: Instant,
    ttl: Duration,
    body: Arc<str>,
    etag: Arc<str>,
}

pub struct Fresh {
    pub body: Arc<str>,
    pub etag: Arc<str>,
    pub max_age: Duration,
}

fn etag_for(body: &str) -> Arc<str> {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    format!("\"{:016x}\"", hasher.finish()).into()
}

#[derive(Clone)]
pub struct Cached {
    held: Arc<RwLock<Option<Held>>>,
    ttl: Duration,
}

impl Cached {
    pub fn new(ttl: Duration) -> Self {
        Self {
            held: Arc::new(RwLock::new(None)),
            ttl,
        }
    }

    fn fresh(&self, held: &Option<Held>) -> Option<Fresh> {
        let held = held.as_ref()?;
        held.ttl
            .checked_sub(held.built.elapsed())
            .filter(|left| !left.is_zero())
            .map(|max_age| Fresh {
                body: Arc::clone(&held.body),
                etag: Arc::clone(&held.etag),
                max_age,
            })
    }

    pub async fn get_or_build<B, F, E>(&self, build: B) -> Result<Fresh, E>
    where
        B: FnOnce() -> F,
        F: Future<Output = Result<String, E>>,
    {
        self.get_or_build_capped(Duration::MAX, build).await
    }

    pub async fn get_or_build_capped<B, F, E>(&self, cap: Duration, build: B) -> Result<Fresh, E>
    where
        B: FnOnce() -> F,
        F: Future<Output = Result<String, E>>,
    {
        if let Some(cached) = self.fresh(&*self.held.read().await) {
            return Ok(cached);
        }

        let mut held = self.held.write().await;
        if let Some(cached) = self.fresh(&held) {
            return Ok(cached);
        }

        let ttl = self.ttl.min(cap);
        let body: Arc<str> = build().await?.into();
        let etag = etag_for(&body);
        *held = Some(Held {
            built: Instant::now(),
            ttl,
            body: Arc::clone(&body),
            etag: Arc::clone(&etag),
        });

        Ok(Fresh {
            body,
            etag,
            max_age: ttl,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counting(
        builds: &AtomicUsize,
    ) -> impl FnOnce() -> std::future::Ready<Result<String, ()>> + '_ {
        move || {
            builds.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok("<urlset/>".to_owned()))
        }
    }

    #[tokio::test]
    async fn builds_once_and_then_reuses_it_until_the_ttl_runs_out() {
        let cache = Cached::new(Duration::from_secs(60));
        let builds = AtomicUsize::new(0);

        for _ in 0..5 {
            let fresh = cache.get_or_build(counting(&builds)).await.unwrap();
            assert_eq!(&*fresh.body, "<urlset/>");
        }

        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "only the first should build"
        );
    }

    #[tokio::test]
    async fn a_reused_body_is_only_offered_for_the_life_it_has_left() {
        let cache = Cached::new(Duration::from_secs(60));
        let builds = AtomicUsize::new(0);

        let built = cache.get_or_build(counting(&builds)).await.unwrap();
        assert_eq!(built.max_age, Duration::from_secs(60), "freshly built");

        tokio::time::sleep(Duration::from_millis(30)).await;
        let reused = cache.get_or_build(counting(&builds)).await.unwrap();

        assert!(
            reused.max_age < Duration::from_secs(60),
            "a body already part way through its life cannot be offered the whole of it"
        );
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_reused_body_keeps_the_validator_it_was_built_with() {
        let cache = Cached::new(Duration::from_secs(60));
        let builds = AtomicUsize::new(0);

        let built = cache.get_or_build(counting(&builds)).await.unwrap();
        let reused = cache.get_or_build(counting(&builds)).await.unwrap();

        assert_eq!(built.etag, reused.etag, "same body, same validator");
        assert!(built.etag.starts_with('"') && built.etag.ends_with('"'));
    }

    #[tokio::test]
    async fn a_different_body_gets_a_different_validator() {
        assert_ne!(etag_for("<urlset/>"), etag_for("<urlset><url/></urlset>"));
    }

    #[tokio::test]
    async fn a_cap_shorter_than_the_ttl_is_the_one_that_counts() {
        let cache = Cached::new(Duration::from_secs(3600));
        let builds = AtomicUsize::new(0);

        let built = cache
            .get_or_build_capped(Duration::from_secs(30), counting(&builds))
            .await
            .unwrap();

        assert_eq!(
            built.max_age,
            Duration::from_secs(30),
            "a feed built half a minute before an entry is due must not be offered for an hour"
        );
    }

    #[tokio::test]
    async fn a_cap_longer_than_the_ttl_changes_nothing() {
        let cache = Cached::new(Duration::from_secs(3600));
        let builds = AtomicUsize::new(0);

        let built = cache
            .get_or_build_capped(Duration::from_secs(86_400), counting(&builds))
            .await
            .unwrap();

        assert_eq!(built.max_age, Duration::from_secs(3600));
    }

    #[tokio::test]
    async fn a_capped_body_is_dropped_when_its_cap_runs_out_not_when_the_ttl_would() {
        let cache = Cached::new(Duration::from_secs(3600));
        let builds = AtomicUsize::new(0);

        cache
            .get_or_build_capped(Duration::ZERO, counting(&builds))
            .await
            .unwrap();
        cache
            .get_or_build_capped(Duration::ZERO, counting(&builds))
            .await
            .unwrap();

        assert_eq!(
            builds.load(Ordering::SeqCst),
            2,
            "the hour-long TTL must not keep a body alive past the publication that stales it"
        );
    }

    #[tokio::test]
    async fn rebuilds_once_the_entry_is_stale() {
        let cache = Cached::new(Duration::ZERO);
        let builds = AtomicUsize::new(0);

        cache.get_or_build(counting(&builds)).await.unwrap();
        cache.get_or_build(counting(&builds)).await.unwrap();

        assert_eq!(
            builds.load(Ordering::SeqCst),
            2,
            "a zero TTL is always stale"
        );
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_build() {
        let cache = Cached::new(Duration::from_secs(60));
        let builds = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for _ in 0..20 {
            let (cache, builds) = (cache.clone(), Arc::clone(&builds));
            tasks.push(tokio::spawn(async move {
                cache
                    .get_or_build(|| async {
                        builds.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok::<_, ()>("<urlset/>".to_owned())
                    })
                    .await
                    .unwrap()
            }));
        }

        for task in tasks {
            assert_eq!(&*task.await.unwrap().body, "<urlset/>");
        }
        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "twenty callers, one build"
        );
    }
}
