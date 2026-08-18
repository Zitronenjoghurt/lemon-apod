use crate::config::Config;
use anyhow::{Context, Result};
use apod_core::rating::baseline::{Dataset, Manifest, Row};
use apod_core::rating::{self, Category, Grouping, MODEL, Prior, VoteStore};
use apod_core::{ApodReader, ApodWriter, PARSER_VERSION};
use chrono::Utc;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn dataset_dir(cfg: &Config) -> PathBuf {
    cfg.baseline_dir.join("rating")
}

fn read(dir: &Path, category: Category) -> Result<Dataset> {
    let path = dir.join(format!("{category}.csv"));
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;

    Dataset::parse(category, &text).with_context(|| format!("parsing {}", path.display()))
}

pub async fn import(cfg: &Config) -> Result<()> {
    let dir = dataset_dir(cfg);
    let manifest_path = dir.join("manifest.json");
    let manifest = Manifest::parse(
        &std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?,
    )
    .with_context(|| format!("reading {}", manifest_path.display()))?;

    if manifest.parser_version != PARSER_VERSION {
        println!(
            "note: this baseline was built against parser version {}, and this build is on {}",
            manifest.parser_version, PARSER_VERSION
        );
    }

    let store = VoteStore::open(&cfg.votes_db).await?;
    println!("loading {} into {}", dir.display(), cfg.votes_db.display());

    for category in Category::ALL {
        let dataset = read(&dir, category)?;
        anyhow::ensure!(
            manifest.agrees_with(&dataset),
            "{}.csv has {} rows, which is not what the manifest says. Refusing to load a file \
             this build cannot vouch for",
            category,
            dataset.rows.len()
        );

        let loaded = store.import(&dataset).await?;
        println!("  {category}: {loaded} priors");
    }

    store.close().await;
    println!("done. The next fit will start from these and live votes will move them");
    Ok(())
}

pub async fn export(cfg: &Config) -> Result<()> {
    let dir = dataset_dir(cfg);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let index = ApodWriter::open(&cfg.index_db).await?;
    let store = VoteStore::open(&cfg.votes_db).await?;

    let grouping = Grouping::new(index.reader().picture_groups().await?);
    let pool = index.reader().picture_pool(None).await?.len() as u64;

    let mut files = BTreeMap::new();
    let mut votes = 0;

    for category in Category::ALL {
        let log = store.log(category).await?;
        let anchors = store.anchors(category).await?;
        let fit = rating::fit(&log, &grouping, &Prior::weak().anchored(anchors));
        votes += fit.votes as u64;

        let rows: Vec<Row> = fit
            .scores
            .iter()
            .map(|score| Row {
                picture: score.picture,
                category,
                score: score.score,
                ess: effective(score.stderr),
                comparisons: score.comparisons,
            })
            .collect();

        let dataset = Dataset::new(category, rows);
        let path = dir.join(dataset.file_name());
        std::fs::write(&path, dataset.render())
            .with_context(|| format!("writing {}", path.display()))?;

        println!(
            "  {category}: {} pictures from {} votes -> {}",
            dataset.rows.len(),
            fit.votes,
            path.display()
        );
        files.insert(dataset.file_name(), dataset.rows.len());
    }

    let manifest = Manifest {
        votes,
        pool,
        files,
        generated_at: Utc::now(),
        parser_version: PARSER_VERSION,
        ..Manifest::rating(Utc::now(), PARSER_VERSION)
    };

    let path = dir.join("manifest.json");
    std::fs::write(&path, manifest.render())
        .with_context(|| format!("writing {}", path.display()))?;

    store.close().await;
    println!(
        "{} rows written, model {MODEL}",
        manifest.files.values().sum::<usize>()
    );
    Ok(())
}

fn effective(stderr: f64) -> f64 {
    if stderr <= 0.0 {
        return 0.0;
    }
    (stderr.powi(-2) / rating::COMPARISON_INFORMATION).min(1e9)
}

pub async fn status(cfg: &Config) -> Result<()> {
    let index = ApodReader::open(apod_core::db::DbConfig::read_only(&cfg.index_db)).await?;
    let store = VoteStore::open(&cfg.votes_db).await?;

    let pool = index.picture_pool(None).await?.len() as u64;
    println!("{pool} pictures are eligible to be rated");

    for category in Category::ALL {
        let tally = store.tally(category).await?;
        let progress = rating::Progress::of(pool, tally.votes);
        let ranked = store.board_size(category, rating::MIN_COMPARISONS).await?;

        println!(
            "  {category}: {} votes, {} on the board, {} of {} through {}",
            tally.votes,
            ranked,
            progress.done,
            progress.target,
            progress.stage.as_str()
        );
        if let Some(ran_at) = tally.ran_at {
            println!(
                "            last fit {} by {}, side bias {:+.3}",
                ran_at.format("%Y-%m-%d %H:%M:%SZ"),
                tally.model.as_deref().unwrap_or("?"),
                tally.side_bias.unwrap_or(0.0)
            );
        }
    }

    println!(
        "{} voters on record",
        store.tally(Category::Beautiful).await?.voters
    );
    store.close().await;
    Ok(())
}

pub async fn forget(cfg: &Config, voter: &str) -> Result<()> {
    use apod_core::rating::store::VoterId;

    let id = VoterId::from_hex(voter).with_context(|| format!("'{voter}' is not a voter id"))?;

    let store = VoteStore::open(&cfg.votes_db).await?;

    let Some(found) = store.voter(id).await? else {
        println!("no such voter");
        store.close().await;
        return Ok(());
    };

    let kin = match &found.cohort {
        None => Vec::new(),
        Some(cohort) => store.kin(cohort).await?,
    };

    let forgotten = store.forget(id).await?;
    println!("forgot {forgotten} votes");

    if kin.len() > 1 {
        println!(
            "{} other tokens shared their cohort. To take those too:",
            kin.len() - 1
        );
        for other in kin.iter().filter(|other| **other != id) {
            println!("  apod-archiver rating forget {}", other.to_hex());
        }
    }

    println!("run the API's next fit, or `rating export`, to see the board correct itself");
    store.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standard_error_and_a_sample_size_are_the_same_fact_twice() {
        // Ten comparisons is a standard error of about 0.63, and back again.
        assert!((effective(2.0 / 10.0f64.sqrt()) - 10.0).abs() < 1e-6);
        assert!((effective(2.0 / 300.0f64.sqrt()) - 300.0).abs() < 1e-6);
    }

    #[test]
    fn a_picture_with_no_comparisons_is_worth_nothing_rather_than_infinity() {
        assert_eq!(effective(0.0), 0.0);
        assert!(effective(f64::MIN_POSITIVE).is_finite());
    }
}
