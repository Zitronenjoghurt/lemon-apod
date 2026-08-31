use crate::archive::{ArchiveStore, Next, Source};
use crate::client::{Client, Clients};
use crate::config::{Config, Daily};
use crate::fetch::{self, Outcome};
use crate::media::{self, MediaStore};
use crate::modern;
use crate::pictures;
use crate::shutdown::{self, Shutdown};
use crate::thumbs;
use anyhow::Result;
use apod_core::{ApodDate, ApodWriter, Media};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rand::RngExt;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::Instrument;

const MAX_SLEEP: Duration = Duration::from_secs(900);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn run(cfg: Config) -> Result<()> {
    let clients = Clients::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let (stop, shutdown) = Shutdown::channel();
    let mut handles = Vec::new();

    let archive = ArchiveStore::open(&cfg.archive_db).await?;
    let index = ApodWriter::open(&cfg.index_db).await?;

    if !cfg.fetch_legacy {
        tracing::info!("legacy fetching disabled; apod.nasa.gov will not be contacted");
    }
    if !cfg.fetch_modern {
        tracing::info!("modern fetching disabled; science.nasa.gov will not be contacted");
    }

    for source in Source::ALL {
        let enabled = match source {
            Source::Legacy => cfg.fetch_legacy,
            Source::Modern => cfg.fetch_modern,
        };
        if !enabled {
            continue;
        }

        let walk = Walk::new(source, &clients);

        if cfg.backfill.enabled {
            tracing::info!(
                %source,
                pace = %walk.pace(&cfg),
                "backfill starting, newest owed date first"
            );
            handles.push(tokio::spawn(
                backfill(
                    cfg.clone(),
                    walk.clone(),
                    archive.clone(),
                    index.clone(),
                    shutdown.clone(),
                )
                .instrument(tracing::info_span!("backfill", %source)),
            ));
        } else {
            tracing::info!(%source, "backfill disabled");
        }

        if cfg.daily.enabled {
            tracing::info!(
                %source,
                opens = %format!(
                    "{:02}:{:02} {}",
                    cfg.daily.start_hour, cfg.daily.start_minute, cfg.daily.timezone
                ),
                retry = %duration(cfg.daily.interval),
                gives_up_after = %duration(cfg.daily.window),
                "daily poll starting"
            );
            handles.push(tokio::spawn(
                daily(
                    cfg.clone(),
                    walk.clone(),
                    archive.clone(),
                    index.clone(),
                    shutdown.clone(),
                )
                .instrument(tracing::info_span!("daily", %source)),
            ));
        } else {
            tracing::info!(%source, "daily poll disabled");
        }

        if cfg.recheck_per_day > 0 {
            tracing::info!(%source, per_day = cfg.recheck_per_day, "re-check starting");
            handles.push(tokio::spawn(
                recheck(
                    cfg.clone(),
                    walk,
                    archive.clone(),
                    index.clone(),
                    shutdown.clone(),
                )
                .instrument(tracing::info_span!("recheck", %source)),
            ));
        } else {
            tracing::info!(%source, "re-check disabled");
        }
    }

    if cfg.fetch_modern && cfg.modern_refresh_days > 0 {
        tracing::info!(
            every_days = cfg.modern_refresh_days,
            per_window = cfg.modern_per_page,
            a_window_every = %duration(paced(
                &cfg,
                today_in(cfg.daily.timezone),
                Duration::from_secs(u64::from(cfg.modern_refresh_days) * 86_400),
            )),
            "modern refresh starting, spread across the period"
        );
        handles.push(tokio::spawn(
            modern_refresh(
                cfg.clone(),
                clients.source.clone(),
                archive.clone(),
                index.clone(),
                shutdown.clone(),
            )
            .instrument(tracing::info_span!("modern refresh")),
        ));
    } else {
        tracing::info!("modern refresh disabled");
    }

    if cfg.media.enabled {
        tracing::info!(
            every = %range(cfg.media.delay_min, cfg.media.delay_max),
            backoff_max = %duration(cfg.retry_backoff_max),
            "media archive starting"
        );
        handles.push(tokio::spawn(
            media_backfill(
                cfg.clone(),
                clients.media.clone(),
                archive.clone(),
                index.clone(),
                shutdown.clone(),
            )
            .instrument(tracing::info_span!("media")),
        ));
    } else {
        tracing::info!("media archive disabled");
    }

    if cfg.thumbs.enabled {
        tracing::info!(
            every = %range(cfg.thumbs.delay_min, cfg.thumbs.delay_max),
            "thumbnail gap filler starting"
        );
        handles.push(tokio::spawn(
            thumb_backfill(
                cfg.clone(),
                clients.media.clone(),
                archive.clone(),
                index.clone(),
                shutdown.clone(),
            )
            .instrument(tracing::info_span!("thumbs")),
        ));
    } else {
        tracing::info!("thumbnails disabled");
    }

    if cfg.sky.enabled {
        tracing::info!(
            every = %duration(cfg.sky.interval),
            "sky feeds starting"
        );
        handles.push(tokio::spawn(
            crate::sky::run(cfg.clone(), clients.media.clone(), shutdown.clone())
                .instrument(tracing::info_span!("sky")),
        ));
    } else {
        tracing::info!("sky feeds disabled");
    }

    if cfg.notify.enabled {
        tracing::info!(
            topics = ?cfg.notify.topics(),
            every = %duration(cfg.notify.interval),
            "notifications starting"
        );
        handles.push(tokio::spawn(
            crate::notify::run(cfg.clone(), clients.media.clone(), shutdown.clone())
                .instrument(tracing::info_span!("notify")),
        ));
    } else {
        tracing::info!("notifications disabled");
    }

    shutdown::signal().await;
    tracing::info!("shutting down; letting workers finish their current step");
    let _ = stop.send(true);

    for handle in handles {
        match tokio::time::timeout(DRAIN_TIMEOUT, handle).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => tracing::warn!("worker stopped with an error: {error:#}"),
            Ok(Err(error)) => tracing::warn!("worker panicked: {error}"),
            Err(_) => tracing::warn!("worker did not stop within {DRAIN_TIMEOUT:?}"),
        }
    }

    tracing::info!("stopped");
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Reach {
    Only(ApodDate),
    Back(ApodDate),
}

impl Reach {
    fn date(self) -> ApodDate {
        match self {
            Self::Only(date) | Self::Back(date) => date,
        }
    }
}

struct Advance {
    stored: bool,
    wait: Duration,
}

#[derive(Clone)]
struct Walk {
    source: Source,
    clients: Clients,
}

impl Walk {
    fn new(source: Source, clients: &Clients) -> Self {
        Self {
            source,
            clients: clients.clone(),
        }
    }

    fn pace(&self, cfg: &Config) -> String {
        match self.source {
            Source::Legacy => format!(
                "one date every {}",
                range(cfg.backfill.delay_min, cfg.backfill.delay_max)
            ),
            Source::Modern => format!(
                "up to {} dates every {} or more",
                cfg.modern_per_page,
                duration(cfg.modern_delay_min)
            ),
        }
    }

    async fn advance(
        &self,
        cfg: &Config,
        archive: &ArchiveStore,
        index: &ApodWriter,
        reach: Reach,
    ) -> Advance {
        match self.source {
            Source::Legacy => Advance {
                stored: matches!(
                    step(cfg, &self.clients, archive, index, reach.date()).await,
                    Some(Outcome::Stored { .. } | Outcome::Updated { .. })
                ),
                wait: jitter(cfg.backfill.delay_min, cfg.backfill.delay_max),
            },
            Source::Modern => {
                let window = match reach {
                    Reach::Only(date) => modern::Window::only(date),
                    Reach::Back(date) => modern::Window::back_from(date),
                };

                let Some(pass) =
                    modern_step(cfg, &self.clients.source, archive, index, window).await
                else {
                    return Advance {
                        stored: false,
                        wait: MAX_SLEEP,
                    };
                };

                if pass.stored > 0
                    && cfg.thumbs.enabled
                    && let Reach::Only(date) = reach
                {
                    thumbnail_now(cfg, &self.clients.media, &archive.media(), index, date).await;
                }

                Advance {
                    stored: pass.stored > 0,
                    wait: modern::delay(cfg, pass.elapsed),
                }
            }
        }
    }
}

async fn backfill(
    cfg: Config,
    walk: Walk,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let source = walk.source;
    let mut caught_up = false;

    while !shutdown.is_triggered() {
        let today = today_in(cfg.daily.timezone);

        let bound = match archive
            .next_target(today, source, cfg.retry_backoff_max, Utc::now().timestamp())
            .await?
        {
            Next::Fetch(date) => date,
            Next::Waiting(wait) => {
                let wait = wait.min(MAX_SLEEP);
                tracing::warn!(
                    wait = %duration(wait),
                    "every date left has failed; waiting out the retry backoff"
                );
                if !shutdown.sleep(wait).await {
                    break;
                }
                continue;
            }
            Next::Complete => {
                tracing::info!(
                    back_to = %ApodDate::START,
                    looking_again_in = %duration(MAX_SLEEP),
                    "archive is complete; nothing owed"
                );
                if !caught_up {
                    caught_up = true;
                    regroup(&index).await;
                }
                if !shutdown.sleep(MAX_SLEEP).await {
                    break;
                }
                continue;
            }
        };

        caught_up = false;

        let owed = archive.owed(today, source).await?;
        tracing::info!(%bound, dates_owed = owed, "asking for the next batch");

        let advance = walk
            .advance(&cfg, &archive, &index, Reach::Back(bound))
            .await;

        if !tick(
            &mut shutdown,
            advance.wait,
            "pausing before the next request",
        )
        .await
        {
            break;
        }
    }

    Ok(())
}

pub async fn modern_step(
    cfg: &Config,
    client: &Client,
    archive: &ArchiveStore,
    index: &ApodWriter,
    window: modern::Window,
) -> Option<modern::Pass> {
    match modern::fetch_window(cfg, client, archive, index, window).await {
        Ok(pass) => {
            tracing::info!(
                records = pass.records,
                stored = pass.stored,
                unchanged = pass.unchanged,
                absent = pass.absent,
                warned = pass.warned,
                misfiled = pass.misfiled,
                oldest = pass.oldest.map(|date| date.to_string()),
                "modern window recorded"
            );
            Some(pass)
        }
        Err(error) => {
            tracing::warn!("the modern window was not recorded: {error:#}");
            None
        }
    }
}

async fn media_backfill(
    cfg: Config,
    client: Client,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let store = archive.media();
    let mut targets = media_targets(&index).await?;
    let mut scanned = Instant::now();

    while !shutdown.is_triggered() {
        if scanned.elapsed() >= MAX_SLEEP {
            targets = media_targets(&index).await?;
            scanned = Instant::now();
        }

        let target = match store
            .next_target(&targets, cfg.retry_backoff_max, Utc::now().timestamp())
            .await?
        {
            Next::Fetch(target) => target,
            Next::Waiting(wait) => {
                let wait = wait.min(MAX_SLEEP);
                tracing::info!(
                    retrying_in = %duration(wait),
                    "every file that can be fetched is fetched; the rest are waiting on a host \
                     that would not answer"
                );
                if !shutdown.sleep(wait).await {
                    break;
                }
                continue;
            }
            Next::Complete => {
                tracing::info!(
                    files = targets.len(),
                    rescanning_in = %duration(MAX_SLEEP),
                    "every picture the index knows about is stored"
                );
                if !shutdown.sleep(MAX_SLEEP).await {
                    break;
                }
                targets = media_targets(&index).await?;
                scanned = Instant::now();
                continue;
            }
        };

        media_step(&cfg, &client, &store, &targets, &target).await;

        let wait = jitter(cfg.media.delay_min, cfg.media.delay_max);
        if !tick(&mut shutdown, wait, "pausing before the next file").await {
            break;
        }
    }

    Ok(())
}

async fn media_targets(index: &ApodWriter) -> Result<Vec<media::Target>> {
    Ok(media::targets(
        &index.all_media().await?,
        &index.reader().origin_pairs().await?,
    ))
}

async fn thumb_backfill(
    cfg: Config,
    client: Client,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let store = archive.media();
    let mut written_off: HashSet<ApodDate> = HashSet::new();

    while !shutdown.is_triggered() {
        let missing = index.missing_thumbs().await?;
        let outstanding = missing
            .iter()
            .filter(|(date, _)| !written_off.contains(date))
            .count();

        if outstanding > 0 {
            tracing::info!(
                count = outstanding,
                written_off = written_off.len(),
                "filling in missing thumbnails"
            );
        }

        let mut made_any = false;

        for (date, media) in &missing {
            if written_off.contains(date) {
                continue;
            }

            let outcome = thumbnail(&cfg, &client, &store, &index, *date, media).await;
            let paced = match &outcome {
                Some(thumbs::Generated::Written(source)) => {
                    made_any = true;
                    *source == thumbs::Source::Network
                }
                Some(thumbs::Generated::Adopted) => {
                    made_any = true;
                    false
                }
                Some(thumbs::Generated::NotApplicable) => false,
                Some(thumbs::Generated::Failed(_)) | None => {
                    written_off.insert(*date);
                    true
                }
            };

            if paced {
                let wait = jitter(cfg.thumbs.delay_min, cfg.thumbs.delay_max);
                if !tick(&mut shutdown, wait, "pausing before the next thumbnail").await {
                    return Ok(());
                }
            }
        }

        if !made_any {
            tracing::info!(
                written_off = written_off.len(),
                looking_again_in = %duration(MAX_SLEEP),
                "every entry that can have a thumbnail has one"
            );
            if !shutdown.sleep(MAX_SLEEP).await {
                break;
            }
        }
    }

    Ok(())
}

pub async fn media_step(
    cfg: &Config,
    client: &Client,
    store: &MediaStore,
    targets: &[media::Target],
    target: &media::Target,
) -> Option<media::Outcome> {
    let siblings = media::siblings(targets, target.date);
    let outcome = match media::fetch_and_store(cfg, client, store, target, &siblings).await {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(url = %target.url, "storing the media failed: {error:#}");
            return None;
        }
    };

    let date = target.date;
    let role = target.role.as_str();
    match &outcome {
        media::Outcome::Stored { bytes } => tracing::info!(%date, role, bytes, "media stored"),
        media::Outcome::Adopted { bytes } => {
            tracing::debug!(%date, role, bytes, "media already on disk")
        }
        media::Outcome::Missing => tracing::info!(%date, role, url = %target.url, "media is gone"),
        media::Outcome::Rejected(reason) => {
            tracing::warn!(%date, role, %reason, "refusing to store the response")
        }
        media::Outcome::Failed(reason) => tracing::warn!(%date, role, %reason, "media failed"),
    }

    Some(outcome)
}

async fn daily(
    cfg: Config,
    walk: Walk,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let source = walk.source;

    while !shutdown.is_triggered() {
        let now = Utc::now().with_timezone(&cfg.daily.timezone);
        let today = ApodDate::from(now.date_naive());

        if archive
            .get(today, source)
            .await?
            .is_some_and(|record| record.is_success())
        {
            if !sleep_until(
                &mut shutdown,
                next_window(&cfg, now),
                "today is already stored; waiting for tomorrow's window",
            )
            .await
            {
                break;
            }
            continue;
        }

        let Some(window_start) = window_on(&cfg.daily, now.date_naive()) else {
            tracing::warn!(
                %today,
                wait = %duration(MAX_SLEEP),
                "this date has no publication window in the configured timezone"
            );
            if !shutdown.sleep(MAX_SLEEP).await {
                break;
            }
            continue;
        };

        if now < window_start {
            if !sleep_until(
                &mut shutdown,
                window_start.with_timezone(&Utc),
                "waiting for today's publication window to open",
            )
            .await
            {
                break;
            }
            continue;
        }

        if now - window_start > chrono::TimeDelta::from_std(cfg.daily.window)? {
            tracing::warn!(%today, "publication window elapsed without an entry");
            if !sleep_until(
                &mut shutdown,
                next_window(&cfg, now),
                "giving up on today; waiting for tomorrow's window",
            )
            .await
            {
                break;
            }
            continue;
        }

        let advance = walk
            .advance(&cfg, &archive, &index, Reach::Only(today))
            .await;

        if advance.stored {
            regroup(&index).await;
        }

        let kept_waiting = match advance.stored {
            true => {
                sleep_until(
                    &mut shutdown,
                    next_window(&cfg, now),
                    "today is stored; waiting for tomorrow's window",
                )
                .await
            }
            false => {
                tick(
                    &mut shutdown,
                    cfg.daily.interval,
                    "nothing published yet; asking again shortly",
                )
                .await
            }
        };

        if !kept_waiting {
            break;
        }
    }

    Ok(())
}

async fn recheck(
    cfg: Config,
    walk: Walk,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let interval = Duration::from_secs(86_400 / u64::from(cfg.recheck_per_day).max(1));
    let source = walk.source;

    while tick(&mut shutdown, interval, "waiting for the next re-check").await {
        let candidates = archive.recheck_candidates(source, 1).await?;
        tracing::debug!(count = candidates.len(), "re-checking the oldest entries");
        for date in candidates {
            walk.advance(&cfg, &archive, &index, Reach::Only(date))
                .await;
        }
    }

    Ok(())
}

async fn modern_refresh(
    cfg: Config,
    client: Client,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let period = Duration::from_secs(u64::from(cfg.modern_refresh_days) * 86_400);

    loop {
        let today = today_in(cfg.daily.timezone);
        let pace = paced(&cfg, today, period);
        let cutoff = Utc::now().timestamp() - period.as_secs() as i64;

        let wait = match archive.stale_before(Source::Modern, cutoff).await? {
            None => {
                tracing::debug!("every modern record has been looked at inside the period");
                pace
            }
            Some(bound) => {
                match modern_step(
                    &cfg,
                    &client,
                    &archive,
                    &index,
                    modern::Window::back_from(bound),
                )
                .await
                {
                    Some(pass) => pace.max(modern::delay(&cfg, pass.elapsed)),
                    None => pace,
                }
            }
        };

        if !tick(&mut shutdown, wait, "pausing before the next modern window").await {
            break;
        }
    }

    Ok(())
}

fn paced(cfg: &Config, today: ApodDate, period: Duration) -> Duration {
    let dates = u64::try_from(today.days() - ApodDate::START.days())
        .unwrap_or(1)
        .max(1);
    let windows = dates.div_ceil(u64::from(cfg.modern_per_page)).max(1);
    period / u32::try_from(windows).unwrap_or(u32::MAX)
}

pub async fn step(
    cfg: &Config,
    clients: &Clients,
    archive: &ArchiveStore,
    index: &ApodWriter,
    date: ApodDate,
) -> Option<Outcome> {
    let outcome = match fetch::fetch_and_store(cfg, &clients.source, archive, index, date).await {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(%date, "fetch failed: {error:#}");
            return None;
        }
    };

    match &outcome {
        Outcome::Stored { bytes } => tracing::info!(%date, bytes, "stored"),
        Outcome::Updated { bytes } => tracing::info!(%date, bytes, "updated"),
        Outcome::Unchanged => tracing::debug!(%date, "unchanged"),
        Outcome::Absent => tracing::info!(%date, "not published"),
        Outcome::Rejected(reason) => tracing::warn!(%date, %reason, "rejected"),
        Outcome::Redirected { status, location } => tracing::warn!(
            %date,
            status,
            location = location.as_deref().unwrap_or("none"),
            "redirected away from the source; nothing was stored"
        ),
    }

    if cfg.thumbs.enabled && matches!(outcome, Outcome::Stored { .. } | Outcome::Updated { .. }) {
        thumbnail_now(cfg, &clients.media, &archive.media(), index, date).await;
    }

    Some(outcome)
}

async fn thumbnail(
    cfg: &Config,
    client: &Client,
    store: &MediaStore,
    index: &ApodWriter,
    date: ApodDate,
    media: &Media,
) -> Option<thumbs::Generated> {
    let generated = match thumbs::generate(cfg, client, store, index, date, media, false).await {
        Ok(generated) => generated,
        Err(error) => {
            tracing::warn!(%date, "thumbnail failed: {error:#}");
            return None;
        }
    };

    match &generated {
        thumbs::Generated::Written(source) => tracing::debug!(%date, ?source, "thumbnail made"),
        thumbs::Generated::Adopted => tracing::debug!(%date, "thumbnail already on disk"),
        thumbs::Generated::NotApplicable => return Some(generated),
        thumbs::Generated::Failed(reason) => {
            tracing::warn!(%date, %reason, "thumbnail failed");
            return Some(generated);
        }
    }

    if let Err(error) = pictures::store(cfg, index, date).await {
        tracing::warn!(%date, "could not hash the thumbnail: {error:#}");
    }

    Some(generated)
}

async fn thumbnail_now(
    cfg: &Config,
    client: &Client,
    store: &MediaStore,
    index: &ApodWriter,
    date: ApodDate,
) {
    let media = match index.reader().entry(date).await {
        Ok(Some(entry)) => entry.media,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%date, "could not read back the entry: {error:#}");
            return;
        }
    };

    tokio::time::sleep(jitter(cfg.thumbs.delay_min, cfg.thumbs.delay_max)).await;
    thumbnail(cfg, client, store, index, date, &media).await;
}

async fn regroup(index: &ApodWriter) {
    match index.regroup_pictures().await {
        Ok(groups) => tracing::info!(
            pictures = groups.len(),
            "grouped the pictures that have been shown more than once"
        ),
        Err(error) => tracing::warn!("could not group pictures: {error:#}"),
    }
}

async fn tick(shutdown: &mut Shutdown, wait: Duration, doing: &str) -> bool {
    tracing::debug!(wait = %duration(wait), "{doing}");
    shutdown.sleep(wait).await
}

pub fn duration(wait: Duration) -> String {
    match wait.as_secs() {
        seconds if seconds >= 3600 => format!("{:.1}h", seconds as f64 / 3600.0),
        seconds if seconds >= 60 => format!("{:.0}m", seconds as f64 / 60.0),
        seconds => format!("{seconds}s"),
    }
}

fn range(min: Duration, max: Duration) -> String {
    match min >= max {
        true => duration(min),
        false => format!("{} to {}", duration(min), duration(max)),
    }
}

pub fn today_in(tz: Tz) -> ApodDate {
    ApodDate::from(Utc::now().with_timezone(&tz).date_naive())
}

pub fn jitter(min: Duration, max: Duration) -> Duration {
    if min >= max {
        return min;
    }
    Duration::from_millis(rand::rng().random_range(min.as_millis() as u64..=max.as_millis() as u64))
}

pub fn window_on(daily: &Daily, date: chrono::NaiveDate) -> Option<DateTime<Tz>> {
    let naive = date.and_hms_opt(daily.start_hour, daily.start_minute, 0)?;
    daily.timezone.from_local_datetime(&naive).earliest()
}

fn next_window(cfg: &Config, now: DateTime<Tz>) -> DateTime<Utc> {
    let tomorrow = now.date_naive() + chrono::TimeDelta::days(1);
    window_on(&cfg.daily, tomorrow)
        .map(|start| start.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() + chrono::TimeDelta::hours(1))
}

async fn sleep_until(shutdown: &mut Shutdown, target: DateTime<Utc>, doing: &str) -> bool {
    if let Ok(wait) = (target - Utc::now()).to_std() {
        tracing::info!(
            wait = %duration(wait),
            until = %target.format("%Y-%m-%d %H:%M UTC"),
            "{doing}"
        );
    }

    loop {
        let Ok(remaining) = (target - Utc::now()).to_std() else {
            return true;
        };
        if remaining.is_zero() {
            return true;
        }
        if !shutdown.sleep(remaining.min(MAX_SLEEP)).await {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_wait_reads_as_the_unit_a_person_would_say_it_in() {
        assert_eq!(duration(Duration::from_secs(0)), "0s");
        assert_eq!(duration(Duration::from_secs(45)), "45s");
        assert_eq!(duration(Duration::from_secs(59)), "59s");
        assert_eq!(duration(Duration::from_secs(60)), "1m");
        assert_eq!(duration(Duration::from_secs(900)), "15m");
        assert_eq!(duration(Duration::from_secs(3600)), "1.0h");
        assert_eq!(duration(Duration::from_secs(6 * 3600)), "6.0h");
    }

    #[test]
    fn a_pass_over_the_collection_is_spread_across_the_refresh_period() {
        let mut cfg = Config::from_env().unwrap();
        cfg.modern_per_page = 100;

        let today = ApodDate::from_ymd(2026, 8, 31).unwrap();
        let week = Duration::from_secs(7 * 86_400);
        let pace = paced(&cfg, today, week);

        let windows = (today.days() - ApodDate::START.days()) as u64 / 100;
        assert!(
            (85..=95).contains(&(pace.as_secs() / 60)),
            "a hundred-odd windows over a week is about an hour and a half apart, not back to \
             back: {windows} windows, {} minutes apart",
            pace.as_secs() / 60
        );
        assert!(
            pace * u32::try_from(windows).unwrap() <= week,
            "the whole collection has to fit inside the period, not overrun it"
        );
    }

    #[test]
    fn a_jittered_delay_reads_as_the_range_it_draws_from() {
        assert_eq!(
            range(Duration::from_secs(10), Duration::from_secs(30)),
            "10s to 30s"
        );
        assert_eq!(
            range(Duration::from_secs(60), Duration::from_secs(60)),
            "1m",
            "a fixed delay is one number, not a range of one"
        );
    }

    use super::*;

    #[test]
    fn jitter_stays_within_bounds() {
        let min = Duration::from_secs(10);
        let max = Duration::from_secs(30);
        for _ in 0..100 {
            let delay = jitter(min, max);
            assert!(delay >= min && delay <= max, "{delay:?}");
        }
    }

    #[test]
    fn jitter_handles_a_degenerate_range() {
        let fixed = Duration::from_secs(5);
        assert_eq!(jitter(fixed, fixed), fixed);
    }

    fn eastern_midnight() -> Daily {
        Daily {
            enabled: true,
            timezone: Tz::America__New_York,
            start_hour: 0,
            start_minute: 0,
            interval: Duration::from_secs(60),
            window: Duration::from_secs(12 * 3600),
        }
    }

    fn berlin_time(daily: &Daily, y: i32, m: u32, d: u32) -> String {
        window_on(daily, chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap())
            .unwrap()
            .with_timezone(&chrono_tz::Europe::Berlin)
            .format("%H:%M")
            .to_string()
    }

    #[test]
    fn the_window_tracks_eastern_midnight_not_a_fixed_berlin_hour() {
        let daily = eastern_midnight();

        assert_eq!(berlin_time(&daily, 2026, 1, 15), "06:00");
        assert_eq!(berlin_time(&daily, 2026, 3, 15), "05:00");
        assert_eq!(berlin_time(&daily, 2026, 7, 15), "06:00");
    }

    #[tokio::test(start_paused = true)]
    async fn a_long_wait_is_cut_short_by_shutdown() {
        let (stop, mut shutdown) = Shutdown::channel();
        let target = Utc::now() + chrono::TimeDelta::hours(6);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            stop.send(true).unwrap();
        });

        assert!(!sleep_until(&mut shutdown, target, "waiting").await);
    }
}
