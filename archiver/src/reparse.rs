use crate::config::Config;
use crate::entry::{self, Built};
use crate::progress;
use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodWriter, Merged, Provenance};
use std::path::Path;

#[derive(Debug, Default)]
pub struct Report {
    pub parsed: usize,
    pub both: usize,
    pub legacy_only: usize,
    pub modern_only: usize,
    pub divergences: usize,
    pub issues: usize,
    pub lost: Vec<(ApodDate, String)>,
    pub partial: Vec<(ApodDate, String)>,
}

impl Report {
    fn record(&mut self, date: ApodDate, built: Built) -> Option<Merged> {
        self.issues += built.issues.len();
        self.divergences += built.divergences();

        match built.provenance() {
            Some(Provenance::Both) => self.both += 1,
            Some(Provenance::LegacyOnly) => self.legacy_only += 1,
            Some(Provenance::ModernOnly) => self.modern_only += 1,
            None => {}
        }

        let failures: Vec<String> = [
            built.legacy_failed.as_ref().map(|e| format!("legacy: {e}")),
            built.modern_failed.as_ref().map(|e| format!("modern: {e}")),
        ]
        .into_iter()
        .flatten()
        .collect();

        match built.merged {
            Some(merged) => {
                self.parsed += 1;
                if !failures.is_empty() {
                    self.partial.push((date, failures.join("; ")));
                }
                Some(merged)
            }
            None => {
                self.lost.push((
                    date,
                    match failures.is_empty() {
                        true => "no archived file for this date".to_owned(),
                        false => failures.join("; "),
                    },
                ));
                None
            }
        }
    }
}

pub async fn run(cfg: &Config, index: &ApodWriter, dates: &[ApodDate]) -> Result<Report> {
    let mut merged: Vec<Merged> = Vec::with_capacity(dates.len());
    let mut report = Report::default();

    let bar = progress::bar("parsing", dates.len());
    for &date in dates {
        bar.set_message(date.to_string());
        bar.inc(1);

        if let Some(one) = report.record(date, entry::build(cfg, date)) {
            merged.push(one);
        }
    }
    bar.finish_and_clear();

    let writing = progress::spinner("indexing", format!("writing {} entries", merged.len()));
    index
        .upsert_all(&merged)
        .await
        .context("writing the reparsed index")?;
    writing.finish_and_clear();

    Ok(report)
}

pub fn all_dates(cfg: &Config) -> Result<Vec<ApodDate>> {
    let mut dates = archived_dates(&cfg.html_dir, "html")?;
    dates.extend(archived_dates(&cfg.json_dir, "json")?);
    dates.sort_unstable();
    dates.dedup();
    Ok(dates)
}

pub fn archived_dates(dir: &Path, extension: &str) -> Result<Vec<ApodDate>> {
    let mut dates = Vec::new();
    collect(dir, extension, &mut dates)?;
    dates.sort_unstable();
    dates.dedup();
    Ok(dates)
}

fn collect(dir: &Path, extension: &str, out: &mut Vec<ApodDate>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();

        if path.is_dir() {
            collect(&path, extension, out)?;
        } else if path.extension().is_some_and(|ext| ext == extension)
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
        std::fs::write(root.join("2024/03/2024-03-07.json"), "x").unwrap();

        let dates = archived_dates(&root, "html").unwrap();
        assert_eq!(
            dates.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["1995-06-16", "2024-03-05"]
        );
        assert_eq!(
            archived_dates(&root, "json")
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["2024-03-07"],
            "the two sides live under the same date directories and must not be counted together"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_empty_archive_is_not_an_error() {
        assert!(
            archived_dates(Path::new("/nonexistent/apod"), "html")
                .unwrap()
                .is_empty()
        );
    }
}
