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
use apod_core::{ApodDate, ApodWriter};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rand::RngExt;
use std::time::Duration;

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

    if cfg.fetch_legacy && cfg.backfill.enabled {
        handles.push(tokio::spawn(backfill(
            cfg.clone(),
            clients.clone(),
            archive.clone(),
            index.clone(),
            shutdown.clone(),
        )));
    } else {
        tracing::info!("legacy backfill disabled");
    }

    if cfg.fetch_modern && cfg.backfill.enabled {
        handles.push(tokio::spawn(backfill_modern(
            cfg.clone(),
            clients.source.clone(),
            archive.clone(),
            shutdown.clone(),
        )));
    } else {
        tracing::info!("modern backfill disabled");
    }

    for source in Source::ALL {
        let enabled = match source {
            Source::Legacy => cfg.fetch_legacy,
            Source::Modern => cfg.fetch_modern,
        };
        if enabled && cfg.daily.enabled {
            handles.push(tokio::spawn(daily(
                cfg.clone(),
                clients.clone(),
                archive.clone(),
                index.clone(),
                shutdown.clone(),
                source,
            )));
        } else {
            tracing::info!("{source} daily poll disabled");
        }
    }

    if cfg.fetch_legacy && cfg.recheck_per_day > 0 {
        handles.push(tokio::spawn(recheck(
            cfg.clone(),
            clients.clone(),
            archive.clone(),
            index.clone(),
            shutdown.clone(),
        )));
    } else {
        tracing::info!("re-check disabled");
    }

    if cfg.media.enabled {
        handles.push(tokio::spawn(media_backfill(
            cfg.clone(),
            clients.media.clone(),
            archive.clone(),
            index.clone(),
            shutdown.clone(),
        )));
    } else {
        tracing::info!("media archive disabled");
    }

    if cfg.sky.enabled {
        handles.push(tokio::spawn(crate::sky::run(
            cfg.clone(),
            clients.media.clone(),
            shutdown.clone(),
        )));
    } else {
        tracing::info!("sky feeds disabled");
    }

    if cfg.notify.enabled {
        tracing::info!(topics = ?cfg.notify.topics(), "notifications enabled");
        handles.push(tokio::spawn(crate::notify::run(
            cfg.clone(),
            clients.media.clone(),
            shutdown.clone(),
        )));
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

async fn backfill(
    cfg: Config,
    clients: Clients,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let mut caught_up = false;

    while !shutdown.is_triggered() {
        let today = today_in(cfg.daily.timezone);

        let date = match archive
            .next_target(
                today,
                Source::Legacy,
                cfg.retry_backoff_max,
                Utc::now().timestamp(),
            )
            .await?
        {
            Next::Fetch(date) => date,
            Next::Waiting(wait) => {
                tracing::warn!(
                    ?wait,
                    "every date left has failed and is waiting out its retry backoff"
                );
                if !shutdown.sleep(wait.min(MAX_SLEEP)).await {
                    break;
                }
                continue;
            }
            Next::Complete => {
                tracing::info!("archive is complete back to {}", ApodDate::START);
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

        step(&cfg, &clients, &archive, &index, date).await;

        if !shutdown
            .sleep(jitter(cfg.backfill.delay_min, cfg.backfill.delay_max))
            .await
        {
            break;
        }
    }

    Ok(())
}

async fn backfill_modern(
    cfg: Config,
    client: Client,
    archive: ArchiveStore,
    mut shutdown: Shutdown,
) -> Result<()> {
    while !shutdown.is_triggered() {
        let today = today_in(cfg.daily.timezone);

        let bound = match archive
            .next_target(
                today,
                Source::Modern,
                cfg.retry_backoff_max,
                Utc::now().timestamp(),
            )
            .await?
        {
            Next::Fetch(date) => date,
            Next::Waiting(wait) => {
                tracing::warn!(
                    ?wait,
                    "every date left has failed and is waiting out its retry backoff"
                );
                if !shutdown.sleep(wait.min(MAX_SLEEP)).await {
                    break;
                }
                continue;
            }
            Next::Complete => {
                tracing::info!("the modern archive is complete back to {}", ApodDate::START);
                if !shutdown.sleep(MAX_SLEEP).await {
                    break;
                }
                continue;
            }
        };

        let wait =
            match modern_step(&cfg, &client, &archive, modern::Window::back_from(bound)).await {
                Some(pass) => modern::delay(&cfg, pass.elapsed),
                None => MAX_SLEEP,
            };

        if !shutdown.sleep(wait).await {
            break;
        }
    }

    Ok(())
}

pub async fn modern_step(
    cfg: &Config,
    client: &Client,
    archive: &ArchiveStore,
    window: modern::Window,
) -> Option<modern::Pass> {
    match modern::fetch_window(cfg, client, archive, window).await {
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
    let mut targets = media::targets(&index.all_media().await?);

    while !shutdown.is_triggered() {
        let Some(target) = store.next_target(&targets, cfg.media.max_attempts).await? else {
            tracing::info!(
                files = targets.len(),
                "every picture the index knows about is stored"
            );
            if !shutdown.sleep(MAX_SLEEP).await {
                break;
            }
            targets = media::targets(&index.all_media().await?);
            continue;
        };

        media_step(&cfg, &client, &store, &targets, &target).await;

        if !shutdown
            .sleep(jitter(cfg.media.delay_min, cfg.media.delay_max))
            .await
        {
            break;
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
    clients: Clients,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
    source: Source,
) -> Result<()> {
    while !shutdown.is_triggered() {
        let now = Utc::now().with_timezone(&cfg.daily.timezone);
        let today = ApodDate::from(now.date_naive());

        if archive
            .get(today, source)
            .await?
            .is_some_and(|record| record.is_success())
        {
            if !sleep_until(&mut shutdown, next_window(&cfg, now)).await {
                break;
            }
            continue;
        }

        let Some(window_start) = window_on(&cfg.daily, now.date_naive()) else {
            if !shutdown.sleep(MAX_SLEEP).await {
                break;
            }
            continue;
        };

        if now < window_start {
            if !sleep_until(&mut shutdown, window_start.with_timezone(&Utc)).await {
                break;
            }
            continue;
        }

        if now - window_start > chrono::TimeDelta::from_std(cfg.daily.window)? {
            tracing::warn!(%today, %source, "publication window elapsed without an entry");
            if !sleep_until(&mut shutdown, next_window(&cfg, now)).await {
                break;
            }
            continue;
        }

        let fetched = match source {
            Source::Legacy => {
                let stored = matches!(
                    step(&cfg, &clients, &archive, &index, today).await,
                    Some(Outcome::Stored { .. } | Outcome::Updated { .. })
                );
                if stored {
                    regroup(&index).await;
                }
                stored
            }
            Source::Modern => modern_step(
                &cfg,
                &clients.source,
                &archive,
                modern::Window::today(today),
            )
            .await
            .is_some_and(|pass| pass.stored > 0),
        };

        let kept_waiting = match fetched {
            true => sleep_until(&mut shutdown, next_window(&cfg, now)).await,
            false => shutdown.sleep(cfg.daily.interval).await,
        };

        if !kept_waiting {
            break;
        }
    }

    Ok(())
}

async fn recheck(
    cfg: Config,
    clients: Clients,
    archive: ArchiveStore,
    index: ApodWriter,
    mut shutdown: Shutdown,
) -> Result<()> {
    let interval = Duration::from_secs(86_400 / u64::from(cfg.recheck_per_day).max(1));

    while shutdown.sleep(interval).await {
        for date in archive.recheck_candidates(Source::Legacy, 1).await? {
            step(&cfg, &clients, &archive, &index, date).await;
        }
    }

    Ok(())
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
        thumbnail(cfg, &clients.media, &archive.media(), index, date).await;
    }

    Some(outcome)
}

async fn thumbnail(
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

    match thumbs::generate(cfg, client, store, index, date, &media, false).await {
        Ok(thumbs::Generated::Written(source)) => {
            tracing::debug!(%date, ?source, "thumbnail made")
        }
        Ok(thumbs::Generated::Adopted) => tracing::debug!(%date, "thumbnail already on disk"),
        Ok(thumbs::Generated::NotApplicable) => return,
        Ok(thumbs::Generated::Failed(reason)) => {
            tracing::warn!(%date, %reason, "thumbnail failed");
            return;
        }
        Err(error) => {
            tracing::warn!(%date, "thumbnail failed: {error:#}");
            return;
        }
    }

    if let Err(error) = pictures::store(cfg, index, date).await {
        tracing::warn!(%date, "could not hash the thumbnail: {error:#}");
    }
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

async fn sleep_until(shutdown: &mut Shutdown, target: DateTime<Utc>) -> bool {
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

        assert!(!sleep_until(&mut shutdown, target).await);
    }
}
