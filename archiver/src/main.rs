mod archive;
mod client;
mod config;
mod db;
mod fetch;
mod index;
mod reparse;
mod report;
mod shutdown;
mod thumbs;
mod workers;

use anyhow::Result;
use apod_core::ApodDate;
use archive::ArchiveStore;
use clap::{Parser, Subcommand};
use client::Client;
use config::Config;
use index::IndexStore;
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
        Command::Reparse { stale, from, to } => reparse_range(cfg, stale, from, to),
        Command::Thumbs { force, limit } => thumbs(cfg, force, limit).await,
        Command::Quality {
            date,
            warning,
            limit,
        } => {
            let index = IndexStore::open(&cfg.index_db)?;
            report::quality(&index, date, warning.as_deref(), limit)
        }
        Command::Status => {
            let archive = ArchiveStore::open(&cfg.archive_db)?;
            let index = IndexStore::open(&cfg.index_db)?;
            report::status(&cfg, &archive, &index)
        }
    }
}

async fn backfill(cfg: Config, limit: Option<usize>) -> Result<()> {
    let client = Client::new(&cfg.user_agent, cfg.fetch_timeout, cfg.fetch_max_retries)?;
    let mut archive = ArchiveStore::open(&cfg.archive_db)?;
    let mut index = IndexStore::open(&cfg.index_db)?;
    let today = workers::today_in(cfg.daily.timezone);

    let mut done = 0;
    while limit.is_none_or(|limit| done < limit) {
        let Some(date) = archive.next_target(today)? else {
            tracing::info!("archive is complete");
            break;
        };

        workers::step(&cfg, &client, &mut archive, &mut index, date).await;
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
    let mut archive = ArchiveStore::open(&cfg.archive_db)?;
    let mut index = IndexStore::open(&cfg.index_db)?;

    if !force && let Some(record) = archive.get(date)? {
        if record.is_success() {
            println!("{date} is already archived; pass --force to fetch it again");
            return Ok(());
        }
        if record.is_absent() {
            println!("{date} was never published; pass --force to check again");
            return Ok(());
        }
    }

    match workers::step(&cfg, &client, &mut archive, &mut index, date).await {
        Some(outcome) => println!("{date}: {outcome:?}"),
        None => println!("{date}: failed"),
    }
    Ok(())
}

fn reparse_range(
    cfg: Config,
    stale: bool,
    from: Option<ApodDate>,
    to: Option<ApodDate>,
) -> Result<()> {
    let mut index = IndexStore::open(&cfg.index_db)?;

    let mut dates = if stale {
        index.stale_dates()?
    } else {
        reparse::archived_dates(&cfg.html_dir)?
    };
    dates.retain(|date| from.is_none_or(|from| *date >= from) && to.is_none_or(|to| *date <= to));

    println!("reparsing {} entries...", dates.len());
    let report = reparse::run(&cfg, &mut index, &dates)?;

    println!("parsed {}", report.parsed);
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
    let mut index = IndexStore::open(&cfg.index_db)?;

    let targets = if force {
        let dates = index.all_dates()?;
        index.media_for(&dates)?
    } else {
        index.missing_thumbs()?
    };

    let targets = match limit {
        Some(limit) => &targets[..targets.len().min(limit)],
        None => &targets[..],
    };

    println!("generating up to {} thumbnails...", targets.len());
    let (mut written, mut skipped, mut failed) = (0, 0, 0);

    for (index_position, (date, media)) in targets.iter().enumerate() {
        if index_position > 0 {
            tokio::time::sleep(workers::jitter(cfg.thumbs.delay_min, cfg.thumbs.delay_max)).await;
        }

        match thumbs::generate(&cfg, &client, &mut index, *date, media).await? {
            thumbs::Generated::Written => {
                written += 1;
                tracing::info!(%date, "thumbnail written");
            }
            thumbs::Generated::NotApplicable => skipped += 1,
            thumbs::Generated::Failed(reason) => {
                failed += 1;
                tracing::warn!(%date, %reason, "thumbnail failed");
            }
        }
    }

    println!("written {written}, skipped {skipped}, failed {failed}");
    Ok(())
}
