use crate::config::Config;
use crate::progress;
use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodEntry, ApodWriter, parse};
use std::path::Path;

#[derive(Debug, Default)]
pub struct Report {
    pub parsed: usize,
    pub failed: Vec<(ApodDate, String)>,
}

pub async fn run(cfg: &Config, index: &ApodWriter, dates: &[ApodDate]) -> Result<Report> {
    let mut entries: Vec<ApodEntry> = Vec::with_capacity(dates.len());
    let mut report = Report::default();

    let bar = progress::bar("parsing", dates.len());
    for &date in dates {
        bar.set_message(date.to_string());
        bar.inc(1);

        let path = cfg.html_path(date);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                report
                    .failed
                    .push((date, format!("reading {}: {error}", path.display())));
                continue;
            }
        };

        match parse::parse_bytes(date, &bytes) {
            Ok(entry) => entries.push(entry),
            Err(error) => report.failed.push((date, error.to_string())),
        }
    }
    bar.finish_and_clear();

    report.parsed = entries.len();

    let writing = progress::spinner("indexing", format!("writing {} entries", entries.len()));
    index
        .upsert_all(&entries)
        .await
        .context("writing the reparsed index")?;
    writing.finish_and_clear();

    Ok(report)
}

pub fn archived_dates(html_dir: &Path) -> Result<Vec<ApodDate>> {
    let mut dates = Vec::new();
    collect(html_dir, &mut dates)?;
    dates.sort_unstable();
    dates.dedup();
    Ok(dates)
}

fn collect(dir: &Path, out: &mut Vec<ApodDate>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();

        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "html")
            && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            && let Ok(date) = stem.parse::<ApodDate>()
        {
            out.push(date);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_archived_dates_from_the_filesystem_alone() {
        let root = std::env::temp_dir().join("apod-reparse-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("2024/03")).unwrap();
        std::fs::create_dir_all(root.join("1995/06")).unwrap();

        std::fs::write(root.join("2024/03/2024-03-05.html"), "x").unwrap();
        std::fs::write(root.join("1995/06/1995-06-16.html"), "x").unwrap();
        std::fs::write(root.join("2024/03/notes.txt"), "x").unwrap();
        std::fs::write(root.join("2024/03/2024-03-06.html.tmp"), "x").unwrap();

        let dates = archived_dates(&root).unwrap();
        assert_eq!(
            dates.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["1995-06-16", "2024-03-05"]
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_empty_archive_is_not_an_error() {
        assert!(
            archived_dates(Path::new("/nonexistent/apod"))
                .unwrap()
                .is_empty()
        );
    }
}
