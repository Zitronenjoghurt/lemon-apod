use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use apod_core::ApodDate;
use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const RUNS: &str = include_str!("../../../../baseline/gaps/runs.json");

#[derive(Debug, Clone, Deserialize)]
struct Run {
    from: ApodDate,
    to: ApodDate,
    title: String,
    paragraphs: Vec<String>,
    #[serde(default)]
    caveat: Option<String>,
    #[serde(default)]
    source: Option<Source>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Source {
    label: String,
    url: String,
}

#[derive(Debug, Serialize)]
struct Gap {
    date: ApodDate,
    from: ApodDate,
    to: ApodDate,
    days: u32,
    title: String,
    paragraphs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    caveat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<Source>,
    previous: Option<ApodDate>,
    next: Option<ApodDate>,
}

static PARSED: LazyLock<Result<Vec<Run>, String>> = LazyLock::new(|| {
    let runs: Vec<Run> =
        serde_json::from_str(RUNS).map_err(|error| format!("baseline/gaps/runs.json: {error}"))?;

    for run in &runs {
        if run.to < run.from {
            return Err(format!(
                "baseline/gaps/runs.json: the run at {} ends before it starts",
                run.from
            ));
        }
    }

    Ok(runs)
});

fn runs() -> ApiResult<&'static [Run]> {
    match &*PARSED {
        Ok(runs) => Ok(runs),
        Err(problem) => {
            tracing::error!("{problem}");
            Err(ApiError::Unavailable(
                "the record of missing days could not be read".to_owned(),
            ))
        }
    }
}

pub fn describe(date: ApodDate) -> Option<(&'static str, &'static str)> {
    let runs = PARSED.as_ref().ok()?;
    let run = runs.iter().find(|run| run.from <= date && date <= run.to)?;

    Some((run.title.as_str(), run.paragraphs.first()?.as_str()))
}

fn published_before(date: ApodDate) -> Option<ApodDate> {
    let mut walk = date.prev();
    while walk.days() >= 0 {
        if !walk.is_known_missing() {
            return Some(walk);
        }
        walk = walk.prev();
    }
    None
}

fn published_after(date: ApodDate, today: ApodDate) -> Option<ApodDate> {
    let mut walk = date.next();
    while walk <= today {
        if !walk.is_known_missing() {
            return Some(walk);
        }
        walk = walk.next();
    }
    None
}

fn expand(run: &Run, today: ApodDate) -> impl Iterator<Item = Gap> + '_ {
    let days = (run.to.days() - run.from.days() + 1) as u32;

    (run.from.days()..=run.to.days())
        .map(ApodDate::from_days)
        .map(move |date| Gap {
            date,
            from: run.from,
            to: run.to,
            days,
            title: run.title.clone(),
            paragraphs: run.paragraphs.clone(),
            caveat: run.caveat.clone(),
            source: run.source.clone(),
            previous: published_before(run.from),
            next: published_after(run.to, today),
        })
}

async fn get_gaps(State(state): State<ServerState>) -> ApiResult<Response> {
    let today = state
        .store
        .latest()
        .await?
        .map_or_else(ApodDate::today_utc, |entry| entry.date);

    let mut gaps: Vec<Gap> = runs()?
        .iter()
        .flat_map(|run| expand(run, today))
        .collect::<Vec<_>>();
    gaps.sort_by_key(|gap| gap.date);

    Ok(response::cached(3_600, gaps))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_gaps))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> ApodDate {
        ApodDate::from_ymd(2026, 8, 18).unwrap()
    }

    #[test]
    fn the_baseline_file_parses() {
        let runs = PARSED.as_ref().expect("baseline/gaps/runs.json is valid");
        assert!(!runs.is_empty());

        for run in runs {
            assert!(!run.title.trim().is_empty(), "{} has no title", run.from);
            assert!(
                !run.paragraphs.is_empty(),
                "{} has nothing to say",
                run.from
            );
            assert!(
                run.paragraphs.iter().all(|line| !line.trim().is_empty()),
                "{} has an empty paragraph",
                run.from
            );
        }
    }

    #[test]
    fn the_baseline_covers_exactly_the_days_the_archive_calls_missing() {
        let runs = PARSED.as_ref().unwrap();
        let mut written: Vec<ApodDate> = runs
            .iter()
            .flat_map(|run| (run.from.days()..=run.to.days()).map(ApodDate::from_days))
            .collect();
        written.sort();

        let mut known = ApodDate::KNOWN_MISSING.to_vec();
        known.sort();

        assert_eq!(
            written, known,
            "baseline/gaps/runs.json and ApodDate::KNOWN_MISSING disagree about which days are missing"
        );
    }

    #[test]
    fn a_run_expands_to_one_entry_per_day_all_pointing_past_the_run() {
        let run = Run {
            from: ApodDate::from_ymd(1995, 6, 17).unwrap(),
            to: ApodDate::from_ymd(1995, 6, 19).unwrap(),
            title: "Three days".into(),
            paragraphs: vec!["Nothing here.".into()],
            caveat: None,
            source: None,
        };

        let gaps: Vec<Gap> = expand(&run, today()).collect();
        assert_eq!(gaps.len(), 3);

        for gap in &gaps {
            assert_eq!(gap.days, 3);
            assert_eq!(gap.previous, Some(ApodDate::START));
            assert_eq!(
                gap.next,
                ApodDate::from_ymd(1995, 6, 20),
                "the middle of the run is not somewhere to send a reader"
            );
        }
    }

    #[test]
    fn the_lone_gap_points_at_the_days_either_side_of_it() {
        let date = ApodDate::from_ymd(2020, 6, 10).unwrap();

        assert_eq!(published_before(date), ApodDate::from_ymd(2020, 6, 9));
        assert_eq!(
            published_after(date, today()),
            ApodDate::from_ymd(2020, 6, 11)
        );
    }

    #[test]
    fn nothing_is_offered_past_the_latest_entry() {
        let date = ApodDate::from_ymd(2020, 6, 10).unwrap();
        assert_eq!(published_after(date, date), None);
    }
}
