use super::fit::Anchor;
use super::model::Category;
use super::{BASELINE_MAX_ESS, MODEL};
use crate::date::ApodDate;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: u32 = 1;
pub const HEADER: &str = "date,category,score,ess,comparisons";

const SCORE_PLACES: usize = 3;
const ESS_PLACES: usize = 1;

#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    #[error("expected the header '{HEADER}', found '{0}'")]
    Header(String),
    #[error("line {line}: {reason}")]
    Row { line: usize, reason: String },
    #[error("this file holds {found} rows, but it is named for {expected}")]
    Category { expected: Category, found: Category },
    #[error("{picture} appears more than once")]
    Repeated { picture: ApodDate },
    #[error("schema version {found}, but this build reads {SCHEMA}")]
    Schema { found: u32 },
    #[error("the manifest is not readable JSON")]
    Manifest(#[from] serde_json::Error),
}

pub type BaselineResult<T> = Result<T, BaselineError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    pub picture: ApodDate,
    pub category: Category,
    pub score: f64,
    pub ess: f64,
    pub comparisons: u32,
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub category: Category,
    pub rows: Vec<Row>,
}

impl Dataset {
    pub fn new(category: Category, mut rows: Vec<Row>) -> Self {
        rows.sort_unstable_by_key(|row| row.picture);
        Self { category, rows }
    }

    pub fn empty(category: Category) -> Self {
        Self {
            category,
            rows: Vec::new(),
        }
    }

    pub fn file_name(&self) -> String {
        format!("{}.csv", self.category)
    }

    pub fn render(&self) -> String {
        let mut out = String::with_capacity(HEADER.len() + self.rows.len() * 40);
        out.push_str(HEADER);
        out.push('\n');

        for row in &self.rows {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                row.picture,
                row.category,
                fixed(row.score, SCORE_PLACES),
                fixed(row.ess, ESS_PLACES),
                row.comparisons
            ));
        }

        out
    }

    pub fn parse(category: Category, text: &str) -> BaselineResult<Self> {
        let mut lines = text.lines().enumerate();

        let header = lines
            .next()
            .map_or("", |(_, line)| line.trim_end_matches('\r'));
        if header.trim() != HEADER {
            return Err(BaselineError::Header(header.to_owned()));
        }

        let mut rows = Vec::new();
        for (index, raw) in lines {
            let line = index + 1;
            let raw = raw.trim_end_matches('\r').trim();
            if raw.is_empty() {
                continue;
            }

            let row = parse_row(line, raw)?;
            if row.category != category {
                return Err(BaselineError::Category {
                    expected: category,
                    found: row.category,
                });
            }
            rows.push(row);
        }

        rows.sort_unstable_by_key(|row| row.picture);
        if let Some(pair) = rows
            .windows(2)
            .find(|pair| pair[0].picture == pair[1].picture)
        {
            return Err(BaselineError::Repeated {
                picture: pair[0].picture,
            });
        }

        Ok(Self { category, rows })
    }

    pub fn anchors(&self) -> impl Iterator<Item = Anchor> + '_ {
        self.rows.iter().map(|row| Anchor {
            picture: row.picture,
            score: row.score,
            ess: row.ess.clamp(0.0, BASELINE_MAX_ESS),
        })
    }
}

fn parse_row(line: usize, raw: &str) -> BaselineResult<Row> {
    let fail = |reason: &str| BaselineError::Row {
        line,
        reason: reason.to_owned(),
    };

    let mut fields = raw.split(',');
    let mut next = |what: &str| fields.next().ok_or_else(|| fail(&format!("no {what}")));

    let picture: ApodDate = next("date")?
        .trim()
        .parse()
        .map_err(|_| fail("the date is not YYYY-MM-DD"))?;
    let category: Category = next("category")?
        .trim()
        .parse()
        .map_err(|_| fail("unknown category"))?;
    let score: f64 = next("score")?
        .trim()
        .parse()
        .map_err(|_| fail("the score is not a number"))?;
    let ess: f64 = next("ess")?
        .trim()
        .parse()
        .map_err(|_| fail("the effective sample size is not a number"))?;
    let comparisons: u32 = next("comparisons")?
        .trim()
        .parse()
        .map_err(|_| fail("the comparison count is not a whole number"))?;

    if fields.next().is_some() {
        return Err(fail("more fields than the header names"));
    }
    if !score.is_finite() || !ess.is_finite() || ess < 0.0 {
        return Err(fail("a score or sample size that cannot be believed"));
    }

    Ok(Row {
        picture,
        category,
        score,
        ess,
        comparisons,
    })
}

/// What an import needs to know about what it is loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub dataset: String,
    /// Which key the rows are addressed by. Rating keys on the picture group; per-entry datasets
    /// key on the entry. Both are dates and they are not the same key.
    pub key: String,
    pub producer: String,
    pub generated_at: DateTime<Utc>,
    pub parser_version: u32,
    pub votes: u64,
    pub pool: u64,
    /// Row counts per file, so a truncated file is caught before it is loaded.
    pub files: BTreeMap<String, usize>,
}

impl Manifest {
    pub fn rating(generated_at: DateTime<Utc>, parser_version: u32) -> Self {
        Self {
            schema: SCHEMA,
            dataset: "rating".to_owned(),
            key: "picture_group".to_owned(),
            producer: MODEL.to_owned(),
            generated_at,
            parser_version,
            votes: 0,
            pool: 0,
            files: Category::ALL
                .iter()
                .map(|category| (format!("{category}.csv"), 0))
                .collect(),
        }
    }

    pub fn render(&self) -> String {
        let mut out = serde_json::to_string_pretty(self).expect("a manifest always serialises");
        out.push('\n');
        out
    }

    pub fn parse(text: &str) -> BaselineResult<Self> {
        let manifest: Self = serde_json::from_str(text)?;
        if manifest.schema != SCHEMA {
            return Err(BaselineError::Schema {
                found: manifest.schema,
            });
        }
        Ok(manifest)
    }

    pub fn agrees_with(&self, dataset: &Dataset) -> bool {
        self.files
            .get(&dataset.file_name())
            .is_some_and(|&rows| rows == dataset.rows.len())
    }
}

fn fixed(value: f64, places: usize) -> String {
    let rendered = format!("{value:.places$}");
    let unsigned = rendered.trim_start_matches('-');

    match unsigned.chars().all(|digit| digit == '0' || digit == '.') {
        true => unsigned.to_owned(),
        false => rendered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> ApodDate {
        text.parse().unwrap()
    }

    fn row(date: &str, score: f64, ess: f64) -> Row {
        Row {
            picture: day(date),
            category: Category::Beautiful,
            score,
            ess,
            comparisons: 63,
        }
    }

    #[test]
    fn an_empty_file_is_a_header_and_a_newline() {
        let rendered = Dataset::empty(Category::Beautiful).render();
        assert_eq!(rendered, "date,category,score,ess,comparisons\n");

        let read = Dataset::parse(Category::Beautiful, &rendered).unwrap();
        assert!(read.rows.is_empty());
        assert_eq!(read.anchors().count(), 0);
    }

    #[test]
    fn a_row_survives_a_round_trip_through_the_file() {
        let dataset = Dataset::new(Category::Beautiful, vec![row("1995-08-04", 1.8423, 41.0)]);
        let rendered = dataset.render();

        assert!(
            rendered.contains("1995-08-04,beautiful,1.842,41.0,63"),
            "{rendered}"
        );

        let read = Dataset::parse(Category::Beautiful, &rendered).unwrap();
        assert_eq!(read.rows.len(), 1);
        assert_eq!(read.rows[0].picture, day("1995-08-04"));
        assert!((read.rows[0].score - 1.842).abs() < 1e-9);
    }

    #[test]
    fn rows_come_out_in_date_order_however_they_went_in() {
        let dataset = Dataset::new(
            Category::Beautiful,
            vec![
                row("2020-01-01", 0.5, 10.0),
                row("1995-08-04", 1.0, 10.0),
                row("2005-06-06", -0.5, 10.0),
            ],
        );

        let dates: Vec<String> = dataset
            .rows
            .iter()
            .map(|row| row.picture.to_string())
            .collect();
        assert_eq!(dates, ["1995-08-04", "2005-06-06", "2020-01-01"]);
    }

    #[test]
    fn a_re_export_of_the_same_fit_is_the_same_bytes() {
        let rows = vec![
            row("1995-08-04", 1.842_312, 41.04),
            row("2020-01-01", -0.0004, 9.96),
        ];
        let once = Dataset::new(Category::Beautiful, rows.clone()).render();
        let twice = Dataset::new(Category::Beautiful, rows).render();

        assert_eq!(once, twice);
        assert!(
            !once.contains("-0.000"),
            "a signed zero would show as a diff for a number that did not move: {once}"
        );
    }

    #[test]
    fn an_effective_sample_size_is_capped_on_the_way_in() {
        let dataset = Dataset::new(
            Category::Beautiful,
            vec![
                row("1995-08-04", 2.0, 4_000.0),
                row("1996-01-01", 1.0, 12.0),
            ],
        );

        let anchors: Vec<Anchor> = dataset.anchors().collect();
        assert_eq!(
            anchors[0].ess, BASELINE_MAX_ESS,
            "a fossil is not an argument"
        );
        assert_eq!(anchors[1].ess, 12.0, "and a thin row keeps its own weight");
    }

    #[test]
    fn a_file_copied_to_the_wrong_name_is_refused() {
        let rendered =
            Dataset::new(Category::Beautiful, vec![row("1995-08-04", 1.0, 10.0)]).render();

        let error = Dataset::parse(Category::Fascinating, &rendered).unwrap_err();
        assert!(
            matches!(error, BaselineError::Category { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn a_file_that_is_not_one_of_ours_is_refused_rather_than_half_read() {
        for text in [
            "",
            "date,score\n1995-08-04,1.0\n",
            "date,category,score,ess,comparisons\nnot-a-date,beautiful,1.0,10.0,5\n",
            "date,category,score,ess,comparisons\n1995-08-04,impressive,1.0,10.0,5\n",
            "date,category,score,ess,comparisons\n1995-08-04,beautiful,lovely,10.0,5\n",
            "date,category,score,ess,comparisons\n1995-08-04,beautiful,1.0,-2.0,5\n",
            "date,category,score,ess,comparisons\n1995-08-04,beautiful,1.0,10.0,5,extra\n",
        ] {
            assert!(
                Dataset::parse(Category::Beautiful, text).is_err(),
                "accepted {text:?}"
            );
        }
    }

    #[test]
    fn one_picture_twice_is_refused_because_only_one_of_them_could_be_the_prior() {
        let text = "date,category,score,ess,comparisons\n\
                    1995-08-04,beautiful,1.0,10.0,5\n\
                    1995-08-04,beautiful,2.0,10.0,5\n";

        let error = Dataset::parse(Category::Beautiful, text).unwrap_err();
        assert!(
            matches!(error, BaselineError::Repeated { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn a_blank_line_and_a_windows_ending_are_both_tolerated() {
        let text = "date,category,score,ess,comparisons\r\n\
                    1995-08-04,beautiful,1.0,10.0,5\r\n\
                    \n";

        let read = Dataset::parse(Category::Beautiful, text).unwrap();
        assert_eq!(read.rows.len(), 1);
    }

    #[test]
    fn a_manifest_names_what_it_is_and_what_produced_it() {
        let manifest = Manifest::rating(Utc::now(), 5);
        let rendered = manifest.render();

        assert!(rendered.ends_with('\n'), "a file ends with a newline");
        assert_eq!(Manifest::parse(&rendered).unwrap(), manifest);
        assert_eq!(manifest.producer, MODEL);
        assert_eq!(manifest.key, "picture_group");
        assert_eq!(manifest.files.len(), 2);
    }

    #[test]
    fn the_manifest_keys_come_out_sorted_so_a_rewrite_is_not_a_reshuffle() {
        let rendered = Manifest::rating(Utc::now(), 5).render();
        let beautiful = rendered.find("beautiful.csv").unwrap();
        let fascinating = rendered.find("fascinating.csv").unwrap();
        assert!(beautiful < fascinating);
    }

    #[test]
    fn a_manifest_from_a_schema_this_build_does_not_read_is_refused() {
        let mut manifest = Manifest::rating(Utc::now(), 5);
        manifest.schema = SCHEMA + 1;

        let error = Manifest::parse(&manifest.render()).unwrap_err();
        assert!(
            matches!(error, BaselineError::Schema { .. }),
            "got {error:?}"
        );
        assert!(Manifest::parse("{").is_err());
    }

    #[test]
    fn a_truncated_file_disagrees_with_its_manifest() {
        let mut manifest = Manifest::rating(Utc::now(), 5);
        let dataset = Dataset::new(Category::Beautiful, vec![row("1995-08-04", 1.0, 10.0)]);

        assert!(
            !manifest.agrees_with(&dataset),
            "the manifest still says zero"
        );
        manifest.files.insert(dataset.file_name(), 1);
        assert!(manifest.agrees_with(&dataset));
    }
}
