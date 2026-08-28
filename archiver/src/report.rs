use crate::archive::{ArchiveStore, Next, Source};
use crate::config::Config;
use crate::entry;
use crate::media;
use crate::progress;
use crate::{reparse, workers};
use anyhow::Result;
use apod_core::{ApodDate, ApodReader, ApodWriter, PARSER_VERSION, quality};
use std::collections::BTreeMap;

pub async fn quality(
    cfg: &Config,
    index: &ApodReader,
    date: Option<ApodDate>,
    warning: Option<&str>,
    limit: usize,
) -> Result<()> {
    let dates = match date {
        Some(date) => vec![date],
        None => index.all_dates().await?,
    };

    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0;
    let mut affected = 0;

    let bar = progress::bar("checking", dates.len());
    for date in &dates {
        bar.inc(1);
        let Some(entry) = index.entry(*date).await? else {
            continue;
        };

        let attributed = entry
            .credits
            .is_empty()
            .then(|| std::fs::read(cfg.html_path(*date)).ok())
            .flatten()
            .map(|bytes| apod_core::parse::bytes_attribute_anyone(&bytes));

        let mut found = quality::quality_control(&entry, attributed);
        if entry.provenance.has_modern() {
            found.extend(entry::build(cfg, *date).issues);
        }

        let issues: Vec<_> = found
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
            progress::println(&format!(
                "{date}  {}\n          {}\n          {}",
                issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                entry.title,
                entry.source_url,
            ));
        }
    }
    bar.finish_and_clear();

    if affected > shown {
        println!("... and {} more entries", affected - shown);
    }

    println!("\n{affected} of {} entries have warnings", dates.len());
    for (warning, count) in totals {
        println!("  {count:>6}  {warning}");
    }

    Ok(())
}

pub async fn status(cfg: &Config, archive: &ArchiveStore, index: &ApodWriter) -> Result<()> {
    let scan = progress::spinner("reading", "counting the archive");
    let today = workers::today_in(cfg.daily.timezone);
    let publishable = today.iter_desc().count();
    let on_disk = reparse::archived_dates(&cfg.html_dir, "html")?.len();
    let json_on_disk = reparse::archived_dates(&cfg.json_dir, "json")?.len();
    let now = chrono::Utc::now().timestamp();
    let stored = archive.stored_dates().await?;
    scan.finish_and_clear();

    println!("today ({})       {today}", cfg.daily.timezone);
    println!("publishable dates {publishable}");
    println!();
    println!("archive");
    println!(
        "  dates stored    {stored} ({:.1}%)",
        percent(stored, publishable)
    );
    println!("  html on disk    {on_disk}");
    println!("  json on disk    {json_on_disk}");
    for source in Source::ALL {
        let counts = archive.counts(source).await?;
        println!("  {source}");
        println!(
            "    stored        {} ({:.1}%)",
            counts.stored,
            percent(counts.stored, publishable)
        );
        println!(
            "    {} {}",
            match source {
                Source::Legacy => "not published",
                Source::Modern => "not migrated ",
            },
            counts.absent
        );
        println!("    redirected    {}", counts.redirected);
        println!("    failed        {}", counts.failed);
        println!("    bytes         {}", size(counts.bytes));
        println!(
            "    next target   {}",
            match archive
                .next_target(today, source, cfg.retry_backoff_max, now)
                .await?
            {
                Next::Fetch(date) => date.to_string(),
                Next::Waiting(wait) => format!("waiting {}", workers::duration(wait)),
                Next::Complete => "complete".to_owned(),
            }
        );
    }
    println!();

    let store = archive.media();
    let targets = media::targets(
        &index.all_media().await?,
        &index.reader().origin_pairs().await?,
    );
    let media_counts = store.counts().await?;
    let next_media = store.next_target(&targets, cfg.media.max_attempts).await?;
    println!("media");
    println!(
        "  stored          {} of {} ({:.1}%)",
        media_counts.stored,
        targets.len(),
        percent(media_counts.stored, targets.len())
    );
    println!(
        "  not fetched     {}",
        targets.len() as i64 - media_counts.stored - media_counts.missing - media_counts.failed
    );
    println!("  gone            {}", media_counts.missing);
    println!("  failed          {}", media_counts.failed);
    println!("  bytes           {}", size(media_counts.bytes));
    println!(
        "  next target     {}",
        match &next_media {
            Some(target) => format!("{} {}", target.date, target.url),
            None => "complete".to_owned(),
        }
    );
    let (compared, agree) = origin_agreement(index, &store).await?;
    println!("  origin matches  {agree} of {compared} pictures archived from both hosts");
    println!();

    let indexed = index.reader().count().await?;
    let stale = index.stale_dates().await?.len();
    let thumbnails = index.reader().thumb_count().await?;
    let pictures = index.reader().picture_summary().await?;
    println!("index");
    println!("  entries         {indexed}");
    println!("  thumbnails      {thumbnails}");
    println!("  hashed          {}", pictures.hashed);
    println!(
        "  encores         {} pictures across {} entries",
        pictures.pictures, pictures.entries
    );
    println!("  parser version  {PARSER_VERSION}");
    println!("  stale entries   {stale}");
    for (provenance, count) in index.reader().provenance_counts().await? {
        println!("  {:<16}{count}", provenance.to_string());
    }

    let divergences = index.reader().divergence_counts().await?;
    println!();
    println!("divergences");
    if divergences.is_empty() {
        println!("  none recorded; reparse has not run since the merge landed");
    }
    for (field, count) in &divergences {
        println!("  {field:<17}{count}");
    }

    if pictures.hashed < thumbnails {
        println!(
            "\n{} thumbnails have not been hashed. Run `apod-archiver pictures`",
            thumbnails - pictures.hashed
        );
    }

    if indexed < stored {
        println!(
            "\n{} stored pages are not indexed. Run `apod-archiver reparse`",
            stored - indexed
        );
    }
    if stale > 0 {
        println!(
            "\n{stale} entries predate the current parser. Run `apod-archiver reparse --stale`"
        );
    }

    Ok(())
}

async fn origin_agreement(index: &ApodWriter, store: &media::MediaStore) -> Result<(usize, usize)> {
    let pairs = index.reader().origin_pairs().await?;
    let urls: Vec<String> = pairs
        .iter()
        .flat_map(|(_, legacy, modern)| [legacy.clone(), modern.clone()])
        .collect();
    let hashes = store.hashes(&urls).await?;

    let mut compared = 0;
    let mut agree = 0;
    for (date, legacy, modern) in pairs {
        let (Some(legacy), Some(modern)) = (hashes.get(&legacy), hashes.get(&modern)) else {
            continue;
        };
        compared += 1;
        match legacy == modern {
            true => agree += 1,
            false => tracing::warn!(
                %date,
                "the origin copy is not byte-identical to the legacy one, which decision 8 assumes"
            ),
        }
    }

    Ok((compared, agree))
}

fn size(bytes: i64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;

    match bytes as f64 {
        bytes if bytes >= GB => format!("{:.1} GB", bytes / GB),
        bytes => format!("{:.1} MB", bytes / MB),
    }
}

fn percent(part: i64, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 * 100.0 / whole as f64
}
