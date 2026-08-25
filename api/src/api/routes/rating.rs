use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::client_ip::client_address;
use crate::rating::ballot::{Ballot, BallotError};
use crate::rating::{Denied, Issued, Rating, Who, weighted_category};
use crate::state::ServerState;
use apod_core::rating::store::VoterId;
use apod_core::rating::{
    self, BASELINE_MAX_ESS, Category, MIN_COMPARISONS, MODEL, Outcome, Progress, Ranked, Score, Z,
};
use apod_core::{ApodDate, ApodSummary, Credit, GameEntry};
use axum::Router;
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use axum::routing::{delete, get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

pub const COOKIE: &str = "apod_voter";
const MAX_OFFSET: usize = 20_000;

#[derive(Debug, Deserialize)]
struct BallotQuery {
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VoteBody {
    ballot: String,
    outcome: String,
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BoardQuery {
    category: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Ticket {
    ballot: String,
    category: Category,
    life: u64,
    left: Side,
    right: Side,
}

#[derive(Debug, Serialize)]
struct Side {
    #[serde(flatten)]
    entry: ApodSummary,
    source_url: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    credit: Vec<String>,
    dates: Vec<ApodDate>,
}

#[derive(Debug, Serialize)]
struct Cast {
    outcome: Outcome,
    left: ApodDate,
    right: ApodDate,
    next: Option<Ticket>,
}

#[derive(Debug, Serialize)]
struct BoardRow {
    #[serde(flatten)]
    entry: ApodSummary,
    tier: u32,
    score: f64,
    stderr: f64,
    lower: f64,
    upper: f64,
    comparisons: u32,
    /// Comparisons' worth of evidence inherited from the committed baseline, so a board restored
    /// from one is not reporting nothing behind every score.
    #[serde(skip_serializing_if = "is_nothing")]
    inherited: f64,
    dates: Vec<ApodDate>,
    source_url: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    credit: Vec<String>,
}

fn is_nothing(value: &f64) -> bool {
    *value <= 0.0
}

#[derive(Debug, Serialize)]
struct Board {
    category: Category,
    provisional: bool,
    progress: Progress,
    ranked: u64,
    pool: u64,
    votes: u64,
    voters: u64,
    favourite: Option<ApodDate>,
    min_comparisons: u32,
    model: Option<String>,
    fitted_at: Option<DateTime<Utc>>,
    side_bias: Option<f64>,
    rows: Vec<BoardRow>,
}

#[derive(Debug, Serialize)]
struct Terms {
    cookie: &'static str,
    cookie_days: u64,
    categories: Vec<Category>,
    beautiful_share: u32,
    min_comparisons: u32,
    baseline_max_ess: f64,
    model: &'static str,
    /// The interval multiplier the board ranks and groups on.
    z: f64,
    votes_per_window: u64,
    window_secs: u64,
    per_picture: u32,
}

fn rating(state: &ServerState) -> ApiResult<&Rating> {
    state
        .rating
        .as_deref()
        .ok_or_else(|| ApiError::Unavailable("rating is not running on this deployment".to_owned()))
}

fn category(raw: Option<&str>) -> ApiResult<Category> {
    match raw {
        None => Ok(Category::Beautiful),
        Some(raw) => raw
            .parse()
            .map_err(|_| ApiError::bad_request(format!("'{raw}' is not a category"))),
    }
}

fn outcome(raw: &str) -> ApiResult<Outcome> {
    raw.parse()
        .map_err(|_| ApiError::bad_request("an outcome is one of left, right or tie"))
}

fn who(state: &ServerState, headers: &HeaderMap, address: Option<SocketAddr>) -> ApiResult<Who> {
    let rating = rating(state)?;

    Ok(Who {
        voter: cookie(headers, COOKIE)
            .as_deref()
            .and_then(VoterId::from_hex),
        cohort: rating.cohort(
            client_address(headers, address, state.config.trusted_proxy_hops),
            headers
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok()),
        ),
    })
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| key.trim() == name)
        .map(|(_, value)| value.trim().to_owned())
}

impl From<Denied> for ApiError {
    fn from(denied: Denied) -> Self {
        match denied {
            Denied::Unavailable => Self::Unavailable(denied.to_string()),
            Denied::OverBudget => Self::TooManyRequests(denied.to_string()),
        }
    }
}

impl From<BallotError> for ApiError {
    fn from(error: BallotError) -> Self {
        Self::bad_request(error.to_string())
    }
}

async fn get_ballot(
    State(state): State<ServerState>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<BallotQuery>,
) -> ApiResult<Response> {
    let rating = rating(&state)?;
    let category = category(query.category.as_deref())?;
    let asker = who(&state, &headers, Some(address))?;
    let now = Utc::now();

    let standing = rating.check(&asker, category, now).await?;
    let issued = rating.draw(category, &asker, &standing, &[], now).await?;

    Ok(response::uncached(ticket(&state, rating, &issued).await?))
}

async fn post_vote(
    State(state): State<ServerState>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<VoteBody>,
) -> ApiResult<Response> {
    let rating = rating(&state)?;
    let asker = who(&state, &headers, Some(address))?;
    let outcome = outcome(&body.outcome)?;
    let now = Utc::now();

    let ballot = rating.verify(&body.ballot, asker.voter, now)?;
    rating.check(&asker, ballot.category, now).await?;
    rating.spend(&ballot)?;

    let minted = asker.voter.is_none();
    let voter = asker.voter.unwrap_or_else(fresh_voter);

    rating
        .record(&ballot, voter, asker.cohort.clone(), outcome, now)
        .await?;

    let next = next_ticket(
        &state,
        rating,
        &asker,
        voter,
        &ballot,
        body.category.as_deref(),
        now,
    )
    .await?;

    let mut response = response::uncached(Cast {
        outcome,
        left: ballot.left,
        right: ballot.right,
        next,
    });

    set_voter_cookie(&mut response, &state, voter, minted);
    Ok(response)
}

fn fresh_voter() -> VoterId {
    use rand::RngExt;
    VoterId::new(rand::rng().random())
}

async fn next_ticket(
    state: &ServerState,
    rating: &Rating,
    asker: &Who,
    voter: VoterId,
    voted: &Ballot,
    wanted: Option<&str>,
    now: DateTime<Utc>,
) -> ApiResult<Option<Ticket>> {
    let category = match wanted {
        Some(raw) => category(Some(raw))?,
        None => weighted_category(rating.settings.beautiful_share),
    };

    let asker = Who {
        voter: Some(voter),
        cohort: asker.cohort.clone(),
    };

    let Ok(standing) = rating.check(&asker, category, now).await else {
        return Ok(None);
    };

    // The pair just judged is not in the standing yet, and seeing it again immediately would be
    // the one thing a session notices.
    let just_seen = [voted.left, voted.right];
    match rating
        .draw(category, &asker, &standing, &just_seen, now)
        .await
    {
        Ok(issued) => Ok(Some(ticket(state, rating, &issued).await?)),
        Err(_) => Ok(None),
    }
}

async fn ticket(state: &ServerState, rating: &Rating, issued: &Issued) -> ApiResult<Ticket> {
    Ok(Ticket {
        ballot: issued.token.clone(),
        category: issued.ballot.category,
        life: rating.settings.ballot_life.as_secs(),
        left: side(state, issued.ballot.left).await?,
        right: side(state, issued.ballot.right).await?,
    })
}

async fn side(state: &ServerState, picture: ApodDate) -> ApiResult<Side> {
    let entry = state
        .store
        .entry(picture)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Side {
        source_url: picture.source_url(),
        credit: credit_lines(&entry.credits),
        dates: state.store.picture_dates(picture).await?,
        entry: entry.to_summary(),
    })
}

fn credit_lines(credits: &[Credit]) -> Vec<String> {
    credits
        .iter()
        .map(|credit| format!("{}: {}", credit.role, credit.text))
        .collect()
}

async fn get_board(
    State(state): State<ServerState>,
    Query(query): Query<BoardQuery>,
) -> ApiResult<Response> {
    let rating = rating(&state)?;
    let category = category(query.category.as_deref())?;

    let limit = params::limit(
        query.limit,
        rating.settings.board_default_limit,
        rating.settings.board_max_limit,
    );
    let offset = params::offset(query.offset).min(MAX_OFFSET);

    let tally = rating.store.tally(category).await?;
    let pool = rating.pool_size().await;
    let progress = rating.progress(tally.votes).await;

    let ranked = rating.store.board_size(category, MIN_COMPARISONS).await?;
    let scores = rating
        .store
        .board(category, MIN_COMPARISONS, limit as i64, offset as i64)
        .await?;

    // Tiers are only meaningful from the top of the board, so a page further down inherits the
    // tier numbering it would have had rather than restarting at one.
    let ranked_scores: Vec<Score> = scores.iter().map(|ranked| ranked.score).collect();
    let tiers = tier_numbers(rating, category, &ranked_scores, offset).await?;

    let rows = board_rows(&state, &scores, &tiers).await?;

    // Two rows, and only when there is a claim to make. Cheap, and it decides the question from the
    // board rather than from whatever page the caller asked for.
    let crown: Vec<Score> = match progress.stage == rating::Stage::Settled {
        false => Vec::new(),
        true => rating
            .store
            .board(category, MIN_COMPARISONS, 2, 0)
            .await?
            .iter()
            .map(|ranked| ranked.score)
            .collect(),
    };

    Ok(response::cached(
        60,
        Board {
            category,
            provisional: progress.stage != rating::Stage::Settled,
            favourite: favourite(&progress, &crown),
            progress,
            ranked,
            pool,
            votes: tally.votes,
            voters: tally.voters,
            min_comparisons: MIN_COMPARISONS,
            model: tally.model,
            fitted_at: tally.ran_at,
            side_bias: tally.side_bias,
            rows,
        },
    ))
}

async fn tier_numbers(
    rating: &Rating,
    category: Category,
    page: &[Score],
    offset: usize,
) -> ApiResult<Vec<u32>> {
    if offset == 0 {
        return Ok(rating::tiers(page));
    }

    let leading = rating
        .store
        .board(category, MIN_COMPARISONS, offset as i64, 0)
        .await?;
    let whole: Vec<Score> = leading
        .iter()
        .map(|ranked| ranked.score)
        .chain(page.iter().copied())
        .collect();

    Ok(rating::tiers(&whole).split_off(offset))
}

/// A single crowned favourite appears only once the settle stage has separated it. If it never
/// separates then the honest permanent answer is a top tier, which is the more interesting claim
/// anyway.
///
/// `top` is the top of the whole board, never the page in hand. Counting tier one inside a page
/// meant a request for a single row saw a tier of one and crowned it, so a top tier of five reported
/// a winner to anybody who asked for the board one row at a time. The dashboard card asks for zero,
/// which clamps to one, so that was the request the site itself made most often.
fn favourite(progress: &Progress, top: &[Score]) -> Option<ApodDate> {
    if progress.stage != rating::Stage::Settled {
        return None;
    }

    let first = top.first()?;
    // A picture with nothing ranked behind it has not been separated from anything.
    let second = top.get(1)?;

    (second.upper() < first.lower()).then_some(first.picture)
}

/// Two queries for the whole page rather than two per row. Asking per row read every ranked
/// entry's explanation off disk only to reach its credits, which at a hundred rows a page is most
/// of the work in the request.
async fn board_rows(
    state: &ServerState,
    scores: &[Ranked],
    tiers: &[u32],
) -> ApiResult<Vec<BoardRow>> {
    let pictures: Vec<ApodDate> = scores.iter().map(|ranked| ranked.score.picture).collect();

    let entries = state.store.summaries(&pictures).await?;
    let mut runs = state.store.group_dates(&pictures).await?;

    let mut by_date: HashMap<ApodDate, GameEntry> = entries
        .into_iter()
        .map(|found| (found.summary.date, found))
        .collect();

    let mut rows = Vec::with_capacity(scores.len());
    for (ranked, &tier) in scores.iter().zip(tiers) {
        let score = &ranked.score;
        let found = by_date.remove(&score.picture).ok_or(ApiError::NotFound)?;

        rows.push(BoardRow {
            tier,
            score: score.score,
            stderr: score.stderr,
            lower: score.lower(),
            upper: score.upper(),
            comparisons: score.comparisons,
            inherited: ranked.inherited,
            dates: runs
                .remove(&score.picture)
                .unwrap_or_else(|| vec![score.picture]),
            source_url: score.picture.source_url(),
            credit: credit_lines(&found.credits),
            entry: found.summary,
        })
    }

    Ok(rows)
}

async fn get_terms(State(state): State<ServerState>) -> ApiResult<Response> {
    let rating = rating(&state)?;

    Ok(response::cached(
        3_600,
        Terms {
            cookie: COOKIE,
            cookie_days: rating.settings.cookie_life.as_secs() / 86_400,
            categories: Category::ALL.to_vec(),
            beautiful_share: rating.settings.beautiful_share,
            min_comparisons: MIN_COMPARISONS,
            baseline_max_ess: BASELINE_MAX_ESS,
            model: MODEL,
            z: Z,
            votes_per_window: rating.settings.votes_per_window,
            window_secs: rating.settings.budget_window.as_secs(),
            per_picture: rating.settings.per_picture,
        },
    ))
}

/// Erasure, offered rather than merely available: it is the same statement as evicting a troll, so
/// the endpoint has to exist either way and putting a button on it costs a paragraph.
async fn delete_votes(
    State(state): State<ServerState>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let rating = rating(&state)?;
    let asker = who(&state, &headers, Some(address))?;

    let forgotten = match asker.voter {
        None => 0,
        Some(voter) => rating.store.forget(voter).await?,
    };

    let mut response = response::uncached(serde_json::json!({ "forgotten": forgotten }));
    clear_voter_cookie(&mut response, &state);
    Ok(response)
}

fn set_voter_cookie(response: &mut Response, state: &ServerState, voter: VoterId, minted: bool) {
    let Some(rating) = state.rating.as_deref() else {
        return;
    };

    let cookie = format!(
        "{COOKIE}={}; Max-Age={}; Path=/; HttpOnly; SameSite=Lax{}",
        voter.to_hex(),
        rating.settings.cookie_life.as_secs(),
        secure(state)
    );

    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    if minted {
        tracing::debug!(?voter, "minted a voter on their first vote");
    }
}

fn clear_voter_cookie(response: &mut Response, state: &ServerState) {
    let cookie = format!(
        "{COOKIE}=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax{}",
        secure(state)
    );

    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

fn secure(state: &ServerState) -> &'static str {
    match state.config.public_url.starts_with("https://") {
        true => "; Secure",
        false => "",
    }
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/ballot", get(get_ballot))
        .route("/vote", post(post_vote))
        .route("/board", get(get_board))
        .route("/terms", get(get_terms))
        .route("/votes", delete(delete_votes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    fn score(picture: i32, score: f64, stderr: f64) -> Score {
        Score {
            picture: ApodDate::from_days(picture),
            score,
            stderr,
            comparisons: 400,
        }
    }

    #[test]
    fn a_voter_cookie_is_found_among_the_others_the_site_sets() {
        let found = cookie(
            &headers(&[("cookie", "theme=dark; apod_voter=abc123; other=1")]),
            COOKIE,
        );
        assert_eq!(found.as_deref(), Some("abc123"));
    }

    #[test]
    fn a_cookie_spread_over_several_headers_is_still_found() {
        let found = cookie(
            &headers(&[("cookie", "theme=dark"), ("cookie", "apod_voter=abc123")]),
            COOKIE,
        );
        assert_eq!(found.as_deref(), Some("abc123"));
    }

    #[test]
    fn no_cookie_means_no_voter_rather_than_a_guess() {
        assert_eq!(cookie(&HeaderMap::new(), COOKIE), None);
        assert_eq!(cookie(&headers(&[("cookie", "theme=dark")]), COOKIE), None);
        assert_eq!(
            cookie(&headers(&[("cookie", "apod_voternot=x")]), COOKIE),
            None
        );
    }

    #[test]
    fn a_category_defaults_to_the_board_that_is_being_filled_first() {
        assert_eq!(category(None).unwrap(), Category::Beautiful);
        assert_eq!(
            category(Some("fascinating")).unwrap(),
            Category::Fascinating
        );
        assert!(category(Some("impressive")).is_err());
    }

    #[test]
    fn an_outcome_is_one_of_three_things_and_nothing_else() {
        assert_eq!(outcome("left").unwrap(), Outcome::Left);
        assert_eq!(outcome("tie").unwrap(), Outcome::Tie);
        assert!(outcome("winner").is_err());
    }

    #[test]
    fn there_is_no_crowned_favourite_until_the_funnel_is_finished() {
        let scores = [score(0, 2.0, 0.05), score(1, 1.0, 0.05)];

        let screening = Progress::of(9_582, 100);
        assert_eq!(favourite(&screening, &scores), None);

        let settled = Progress::of(9_582, 200_000);
        assert_eq!(favourite(&settled, &scores), Some(ApodDate::from_days(0)));
    }

    #[test]
    fn a_single_row_does_not_crown_itself() {
        let settled = Progress::of(9_582, 200_000);
        let alone = [score(0, 2.0, 0.05)];

        assert_eq!(
            favourite(&settled, &alone),
            None,
            "one row says nothing about whether the top of the board separated"
        );
        assert_eq!(favourite(&settled, &[]), None);
    }

    #[test]
    fn a_top_tier_of_five_is_five_rather_than_a_winner_and_four_losers() {
        let scores: Vec<Score> = (0..5)
            .map(|at| score(at, 2.0 - at as f64 * 0.01, 0.1))
            .collect();

        assert_eq!(rating::tiers(&scores), vec![1, 1, 1, 1, 1]);
        assert_eq!(
            favourite(&Progress::of(9_582, 200_000), &scores),
            None,
            "five that cannot be separated is a true statement and a more interesting one"
        );
    }

    #[test]
    fn a_page_further_down_the_board_keeps_the_tier_numbering_it_earned() {
        let whole: Vec<Score> = vec![
            score(0, 3.0, 0.05),
            score(1, 2.0, 0.05),
            score(2, 1.0, 0.05),
            score(3, 0.0, 0.05),
        ];

        let numbering = rating::tiers(&whole);
        assert_eq!(numbering, vec![1, 2, 3, 4]);
        assert_eq!(
            numbering.clone().split_off(2),
            vec![3, 4],
            "the second page is tiers three and four, not one and two again"
        );
    }
}
