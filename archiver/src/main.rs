mod archive;
mod client;
mod config;
mod fetch;
mod reparse;
mod report;
mod shutdown;
mod thumbs;
mod video;
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
        Command::Reparse { stale, from, to } => reparse_range(cfg, stale, from, to).await,
        Command::Thumbs { force, limit } => thumbs(cfg, force, limit).await,
        Command::Quality {
            date,
            warning,
            limit,
        } => {
            let index = ApodWriter::open(&cfg.index_db).await?;
            report::quality(index.reader(), date, warning.as_deref(), limit).await
        }
        Command::Status => {
            let archive = ArchiveStore::open(&cfg.archive_db).await?;
            let index = ApodWriter::open(&cfg.index_db).await?;
            report::status(&cfg, &archive, &index).await
        }
    }
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
    Ok(())
}
