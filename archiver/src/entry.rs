use crate::config::Config;
use anyhow::{Context, Result};
use apod_core::quality::QualityIssue;
use apod_core::{ApodDate, ApodEntry, ApodWriter, Merged, Provenance, merge, parse};
use std::path::Path;

enum Side {
    Missing,
    Failed(String),
    Read(Box<ApodEntry>),
}

impl Side {
    fn exists(&self) -> bool {
        !matches!(self, Self::Missing)
    }

    fn entry(self) -> Option<ApodEntry> {
        match self {
            Self::Read(entry) => Some(*entry),
            _ => None,
        }
    }

    fn failure(&self) -> Option<String> {
        match self {
            Self::Failed(error) => Some(error.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Built {
    pub merged: Option<Merged>,
    pub issues: Vec<QualityIssue>,
    pub legacy_failed: Option<String>,
    pub modern_failed: Option<String>,
}

impl Built {
    pub fn divergences(&self) -> usize {
        self.merged
            .as_ref()
            .map_or(0, |merged| merged.divergences.len())
    }

    pub fn provenance(&self) -> Option<Provenance> {
        self.merged.as_ref().map(|merged| merged.entry.provenance)
    }
}

pub fn build(cfg: &Config, date: ApodDate) -> Built {
    let legacy = side(&cfg.html_path(date), |bytes| {
        parse::parse_bytes(date, bytes).map(|entry| (entry, Vec::new()))
    });
    let modern = side(&cfg.json_path(date), |bytes| {
        parse::parse_json_bytes(date, bytes).map(|read| (read.entry, read.issues))
    });

    let on_disk = (legacy.0.exists(), modern.0.exists());
    let mut built = Built {
        issues: modern.1,
        legacy_failed: legacy.0.failure(),
        modern_failed: modern.0.failure(),
        merged: None,
    };

    built.merged = merge(legacy.0.entry(), modern.0.entry()).map(|mut merged| {
        if let Some(provenance) = Provenance::of(on_disk.0, on_disk.1) {
            merged.entry.provenance = provenance;
        }
        merged
    });

    built
}

fn side<E: std::fmt::Display>(
    path: &Path,
    read: impl Fn(&[u8]) -> Result<(ApodEntry, Vec<QualityIssue>), E>,
) -> (Side, Vec<QualityIssue>) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Side::Missing, Vec::new());
        }
        Err(error) => {
            return (
                Side::Failed(format!("reading {}: {error}", path.display())),
                Vec::new(),
            );
        }
    };

    match read(&bytes) {
        Ok((entry, issues)) => (Side::Read(Box::new(entry)), issues),
        Err(error) => (Side::Failed(error.to_string()), Vec::new()),
    }
}

pub async fn reindex(cfg: &Config, index: &ApodWriter, date: ApodDate) -> Result<Built> {
    let built = build(cfg, date);

    if let Some(merged) = &built.merged {
        index
            .upsert_all(std::slice::from_ref(merged))
            .await
            .with_context(|| format!("indexing {date}"))?;
    }

    for (source, error) in [
        ("legacy", &built.legacy_failed),
        ("modern", &built.modern_failed),
    ] {
        if let Some(error) = error {
            tracing::warn!(%date, source, "stored but could not be read: {error}");
        }
    }

    Ok(built)
}
