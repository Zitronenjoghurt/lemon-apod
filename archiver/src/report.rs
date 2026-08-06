use crate::archive::ArchiveStore;
use crate::config::Config;
use crate::index::IndexStore;
use crate::{reparse, workers};
use anyhow::Result;
use apod_core::{ApodDate, PARSER_VERSION, quality};
use std::collections::BTreeMap;

pub fn quality(
    index: &IndexStore,
    date: Option<ApodDate>,
    warning: Option<&str>,
    limit: usize,
) -> Result<()> {
    let dates = match date {
        Some(date) => vec![date],
        None => index.all_dates()?,
    };

    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0;
    let mut affected = 0;

    for date in &dates {
        let Some(entry) = index.get(*date)? else {
            continue;
        };

        let issues: Vec<_> = quality::quality_control(&entry)
            .into_iter()
            .filter(|issue| warning.is_none_or(|want| issue.warning.to_string() == want))
            .collect();

        if issues.is_empty() {
            continue;
        }
        affected += 1;

        for issue in &issues {
            *totals.entry(issue.warning.to_string()).or_default() += 1;
        }

        if shown < limit {
            shown += 1;
            println!(
                "{date}  {}",
                issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("          {}", entry.title);
            println!("          {}", entry.source_url);
        }
    }

    if affected > shown {
        println!("... and {} more entries", affected - shown);
    }

    println!("\n{affected} of {} entries have warnings", dates.len());
    for (warning, count) in totals {
        println!("  {count:>6}  {warning}");
    }

    Ok(())
}

pub fn status(cfg: &Config, archive: &ArchiveStore, index: &IndexStore) -> Result<()> {
    let today = workers::today_in(cfg.daily.timezone);
    let publishable = today.iter_desc().count();
    let counts = archive.counts()?;
    let on_disk = reparse::archived_dates(&cfg.html_dir)?.len();

    println!("today ({})       {today}", cfg.daily.timezone);
    println!("publishable dates {publishable}");
    println!();
    println!("archive");
    println!(
        "  stored          {} ({:.1}%)",
        counts.stored,
        percent(counts.stored, publishable)
    );
    println!("  not published   {}", counts.absent);
    println!("  failed          {}", counts.failed);
    println!("  html on disk    {on_disk}");
    println!(
        "  bytes           {:.1} MB",
        counts.bytes as f64 / 1_048_576.0
    );
    println!(
        "  next target     {}",
        match archive.next_target(today)? {
            Some(date) => date.to_string(),
            None => "complete".to_owned(),
        }
    );
    println!();

    let indexed = index.count()?;
    let stale = index.stale_dates()?.len();
    println!("index");
    println!("  entries         {indexed}");
    println!("  thumbnails      {}", index.thumb_count()?);
    println!("  parser version  {PARSER_VERSION}");
    println!("  stale entries   {stale}");

    if indexed < counts.stored {
        println!(
            "\n{} stored pages are not indexed. Run `apod-archiver reparse`",
            counts.stored - indexed
        );
    }
    if stale > 0 {
        println!(
            "\n{stale} entries predate the current parser. Run `apod-archiver reparse --stale`"
        );
    }

    Ok(())
}

fn percent(part: i64, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 * 100.0 / whole as f64
}
