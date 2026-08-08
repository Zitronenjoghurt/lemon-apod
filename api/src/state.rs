use crate::config::Config;
use crate::meta::Shell;
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
    pub shell: Arc<Shell>,
    pub sitemap: Cached,
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

        Ok(Self {
            shell: Arc::new(Shell::load(&config)?),
            sky: Sky::new(
                config.sky_db.clone(),
                Duration::from_secs(config.cache_sky_secs),
            ),
            sitemap: Cached::new(Duration::from_secs(config.cache_sitemap_secs)),
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
    body: Arc<str>,
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

    fn fresh(&self, held: &Option<Held>) -> Option<Arc<str>> {
        let held = held.as_ref()?;
        (held.built.elapsed() < self.ttl).then(|| Arc::clone(&held.body))
    }

    pub async fn get_or_build<B, F, E>(&self, build: B) -> Result<Arc<str>, E>
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

        let body: Arc<str> = build().await?.into();
        *held = Some(Held {
            built: Instant::now(),
            body: Arc::clone(&body),
        });

        Ok(body)
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
            let body = cache.get_or_build(counting(&builds)).await.unwrap();
            assert_eq!(&*body, "<urlset/>");
        }

        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "only the first should build"
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
            assert_eq!(&*task.await.unwrap(), "<urlset/>");
        }
        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "twenty callers, one build"
        );
    }
}
