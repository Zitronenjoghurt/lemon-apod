mod archive;
mod client;
mod config;
mod fetch;
mod notify;
mod pictures;
mod reparse;
mod report;
mod shutdown;
mod sky;
mod thumbs;
mod video;
mod weather;
mod workers;

use anyhow::Result;
use apod_core::ApodDate;
use apod_core::ApodWriter;
use archive::ArchiveStore;
use clap::{Parser, Subcommand};
use client::Client;
use config::Config;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "apod-archiver", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the service: backfill, daily poll, and re-check. The default.
    Run,

    /// Fetch missing pages now, newest first, then exit.
    Backfill {
        /// Stop after this many pages.
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Fetch one date.
    Fetch {
        date: ApodDate,
        /// Fetch even if the page is already archived.
        #[arg(long)]
        force: bool,
    },

    /// Rebuild the index from the HTML on disk.
    Reparse {
        /// Only entries produced by an older parser version.
        #[arg(long)]
        stale: bool,
        #[arg(long)]
        from: Option<ApodDate>,
        #[arg(long)]
        to: Option<ApodDate>,
    },

    /// Generate thumbnails.
    Thumbs {
        /// Regenerate thumbnails that already exist.
        #[arg(long)]
        force: bool,
        /// Stop after this many.
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Hash thumbnails and work out which entries show the same picture.
    Pictures {
        /// Rehash thumbnails that have been hashed before.
        #[arg(long)]
        force: bool,
        /// List the pictures the archive has shown most often.
        #[arg(long, default_value_t = 10)]
        show: usize,
    },

    /// Report parse warnings, to guide parser refinement.
    Quality {
        #[arg(long)]
        date: Option<ApodDate>,
        /// Only this warning, e.g. `credit_missing`.
        #[arg(long)]
        warning: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },

    /// Poll the launch and space weather feeds once into sky.db, then exit.
    Sky,

    /// Send any notification that is due, then exit.
    Notify {
        /// Record everything currently due as already sent, without sending it. Run this once
        /// before enabling notifications on an existing archive, or the first pass announces
        /// every eclipse inside the lead window at once.
        #[arg(long)]
        seed: bool,
        /// List what would be sent and leave the record untouched.
        #[arg(long)]
        dry_run: bool,
    },

    /// Coverage and index health.
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env()?;

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => workers::run(cfg).await,
        Command::Backfill { limit } => backfill(cfg, limit).await,
        Command::Fetch { date, force } => fetch_one(cfg, date, force).await,
        Command::Reparse { stale, from, to } => reparse_range(cfg, stale, from, to).await,
        Command::Thumbs { force, limit } => thumbs(cfg, force, limit).await,
        Command::Pictures { force, show } => group_pictures(cfg, force, show).await,
        Command::Quality {
            date,
            warning,
            limit,
        } => {
            let index = ApodWriter::open(&cfg.index_db).await?;
            report::quality(index.reader(), date, warning.as_deref(), limit).await
        }
        Command::Sky => sky::poll(&cfg).await,
        Command::Notify { seed, dry_run } => notify_once(cfg, seed, dry_run).await,
        Command::Status => {
            let archive = ArchiveStore::open(&cfg.archive_db).await?;
            let index = ApodWriter::open(&cfg.index_db).await?;
            report::status(&cfg, &archive, &index).await
        }
    }
}

async fn notify_once(cfg: Config, seed: bool, dry_run: bool) -> Result<()> {
    use apod_core::notify::NotifyStore;
    use chrono::Utc;

    let now = Utc::now();
    let topics = cfg.notify.topics();
    if topics.is_empty() {
        println!("no APOD_NTFY_TOPIC_* is set, so there is nowhere to send anything");
        return Ok(());
    }
    println!("ntfy {} topics {}", cfg.notify.base_url, topics.join(", "));

    let store = NotifyStore::open(&cfg.notify_db).await?;

    if dry_run {
        let mut pending = 0;
        for notification in notify::gather(&cfg, now).await? {
            let sent = store
                .is_sent(&notification.topic, &notification.key)
                .await?;
            println!(
                "  [{}] {} {}",
                if sent { "sent" } else { "due " },
                notification.topic,
                notification.key
            );
            if !sent {
                println!("        {}", notification.title);
                if let Some(click) = &notification.click {
                    println!("        -> {click}");
                }
                pending += 1;
            }
        }
        println!(
            "{pending} would be sent, {} on record",
            store.count().await?
        );
        store.close().await;
        return Ok(());
    }

    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let delivery = match seed {
        true => notify::Delivery::Seed,
        false => notify::Delivery::Send,
    };

    let pass = notify::deliver(&cfg, &client, &store, now, delivery).await?;
    match seed {
        true => println!(
            "seeded {} as already dealt with, {} were on record already",
            pass.claimed, pass.already
        ),
        false => println!(
            "sent {}, failed {}, already on record {}",
            pass.sent, pass.failed, pass.already
        ),
    }

    store.close().await;
    Ok(())
}

async fn backfill(cfg: Config, limit: Option<usize>) -> Result<()> {
    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let archive = ArchiveStore::open(&cfg.archive_db).await?;
    let index = ApodWriter::open(&cfg.index_db).await?;
    let today = workers::today_in(cfg.daily.timezone);

    let mut done = 0;
    while limit.is_none_or(|limit| done < limit) {
        let Some(date) = archive.next_target(today).await? else {
            tracing::info!("archive is complete");
            break;
        };

        workers::step(&cfg, &client, &archive, &index, date).await;
        done += 1;

        if limit.is_none_or(|limit| done < limit) {
            tokio::time::sleep(workers::jitter(
                cfg.backfill.delay_min,
                cfg.backfill.delay_max,
            ))
            .await;
        }
    }

    tracing::info!(fetched = done, "backfill finished");
    Ok(())
}

async fn fetch_one(cfg: Config, date: ApodDate, force: bool) -> Result<()> {
    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let archive = ArchiveStore::open(&cfg.archive_db).await?;
    let index = ApodWriter::open(&cfg.index_db).await?;

    if !force && let Some(record) = archive.get(date).await? {
        if record.is_success() {
            println!("{date} is already archived; pass --force to fetch it again");
            return Ok(());
        }
        if record.is_absent() {
            println!("{date} was never published; pass --force to check again");
            return Ok(());
        }
    }

    match workers::step(&cfg, &client, &archive, &index, date).await {
        Some(outcome) => println!("{date}: {outcome:?}"),
        None => println!("{date}: failed"),
    }
    Ok(())
}

async fn reparse_range(
    cfg: Config,
    stale: bool,
    from: Option<ApodDate>,
    to: Option<ApodDate>,
) -> Result<()> {
    let index = ApodWriter::open(&cfg.index_db).await?;

    let mut dates = if stale {
        index.stale_dates().await?
    } else {
        reparse::archived_dates(&cfg.html_dir)?
    };
    dates.retain(|date| from.is_none_or(|from| *date >= from) && to.is_none_or(|to| *date <= to));

    println!("reparsing {} entries...", dates.len());
    let report = reparse::run(&cfg, &index, &dates).await?;
    println!("parsed {}", report.parsed);

    let pictures = index.regroup_pictures().await?;
    println!("{} pictures have run more than once", pictures.len());

    if !report.failed.is_empty() {
        println!("failed {}:", report.failed.len());
        for (date, error) in report.failed.iter().take(50) {
            println!("  {date}  {error}");
        }
        if report.failed.len() > 50 {
            println!("  ... and {} more", report.failed.len() - 50);
        }
    }

    Ok(())
}

async fn thumbs(cfg: Config, force: bool, limit: Option<usize>) -> Result<()> {
    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let index = ApodWriter::open(&cfg.index_db).await?;

    let measured = measure_existing(&cfg, &index).await?;
    if measured > 0 {
        println!("measured {measured} thumbnails already on disk");
    }

    let targets = if force {
        let dates = index.reader().all_dates().await?;
        index.media_for(&dates).await?
    } else {
        index.missing_thumbs().await?
    };

    let targets = match limit {
        Some(limit) => &targets[..targets.len().min(limit)],
        None => &targets[..],
    };

    println!("generating up to {} thumbnails...", targets.len());
    let (mut written, mut adopted, mut skipped, mut failed) = (0, 0, 0, 0);
    let mut pace = false;

    for (date, media) in targets {
        if pace {
            tokio::time::sleep(workers::jitter(cfg.thumbs.delay_min, cfg.thumbs.delay_max)).await;
        }

        let outcome = thumbs::generate(&cfg, &client, &index, *date, media, force).await?;
        pace = outcome.fetched();

        match outcome {
            thumbs::Generated::Written => {
                written += 1;
                tracing::info!(%date, "thumbnail written");
            }
            thumbs::Generated::Adopted => {
                adopted += 1;
                tracing::debug!(%date, "thumbnail already on disk");
            }
            thumbs::Generated::NotApplicable => skipped += 1,
            thumbs::Generated::Failed(reason) => {
                failed += 1;
                tracing::warn!(%date, %reason, "thumbnail failed");
            }
        }
    }

    println!("written {written}, adopted {adopted}, skipped {skipped}, failed {failed}");

    let report = pictures::refresh(&cfg, &index, false).await?;
    println!(
        "hashed {} more thumbnails, {} pictures have run more than once",
        report.hashed,
        report.groups.len()
    );
    Ok(())
}

async fn group_pictures(cfg: Config, force: bool, show: usize) -> Result<()> {
    let index = ApodWriter::open(&cfg.index_db).await?;

    let pending = if force {
        index.stored_thumbs().await?.len()
    } else {
        index.unhashed_thumbs().await?.len()
    };
    println!("hashing {pending} thumbnails...");

    let report = pictures::refresh(&cfg, &index, force).await?;
    println!("hashed {}, failed {}", report.hashed, report.failed);
    println!(
        "{} pictures have run more than once, across {} entries",
        report.groups.len(),
        report.entries()
    );

    let mut ranked: Vec<_> = report.groups.iter().collect();
    ranked.sort_by_key(|group| (std::cmp::Reverse(group.dates.len()), group.id()));

    for group in ranked.iter().take(show) {
        let title = match index.reader().entry(group.id()).await? {
            Some(entry) => entry.title,
            None => String::from("?"),
        };
        println!("  {} runs  {title}", group.dates.len());
        println!(
            "            {}",
            group
                .dates
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("  ")
        );
    }

    Ok(())
}
async fn measure_existing(cfg: &Config, index: &ApodWriter) -> Result<usize> {
    let mut done = 0;

    for (date, stored_path) in index.unmeasured_thumbs().await? {
        let thumb = thumbs::measured(&stored_path, &cfg.thumb_dir.join(&stored_path));
        if thumb.width.is_none() {
            continue;
        }

        index.set_thumb(date, Some(&thumb)).await?;
        done += 1;
    }

    Ok(done)
}
