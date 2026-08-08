pub mod puzzles;
pub mod token;

use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::{ApodDate, ApodSummary, Deal};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use token::Kind;

const MAX_REVEAL: usize = 24;
const PICTURE_CACHE: &str = "public, max-age=31536000, immutable";

#[derive(Debug, Deserialize)]
pub struct PuzzleQuery {
    day: Option<String>,
    rounds: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct Picture {
    picture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}

impl Picture {
    pub fn of(entry: &ApodSummary) -> Self {
        Self {
            picture: token::encode(Kind::Picture, entry.date),
            width: entry.media.thumb_width,
            height: entry.media.thumb_height,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Reveal {
    picture: String,
    #[serde(flatten)]
    entry: ApodSummary,
    dates: Vec<ApodDate>,
    source_url: String,
}

pub struct Setup {
    pub day: Option<ApodDate>,
    pub deal: Deal,
    pub rounds: usize,
}

impl Setup {
    pub fn before(&self) -> Option<ApodDate> {
        self.day
    }
}

fn setup(
    game: &str,
    query: &PuzzleQuery,
    state: &ServerState,
    default_rounds: usize,
    max_rounds: usize,
) -> ApiResult<Setup> {
    let rounds = params::limit(query.rounds, default_rounds, max_rounds);

    let Some(raw) = query.day.as_deref() else {
        return Ok(Setup {
            day: None,
            deal: Deal::loose(),
            rounds,
        });
    };

    let today = params::date(&crate::schedule::Schedule::now(&state.config.publish).today)?;
    let day = if raw == "today" {
        today
    } else {
        params::date(raw)?
    };

    if day > today {
        return Err(ApiError::bad_request(
            "that day has not come round yet in the publishing timezone",
        ));
    }

    Ok(Setup {
        day: Some(day),
        deal: Deal::daily(game, day),
        rounds,
    })
}

async fn get_date(
    State(state): State<ServerState>,
    Query(query): Query<PuzzleQuery>,
) -> ApiResult<Response> {
    let setup = setup("date", &query, &state, 5, 10)?;
    let daily = setup.day.is_some();
    Ok(puzzle(daily, puzzles::date(&state, setup).await?))
}

async fn get_order(
    State(state): State<ServerState>,
    Query(query): Query<PuzzleQuery>,
) -> ApiResult<Response> {
    let setup = setup("order", &query, &state, 10, 20)?;
    let daily = setup.day.is_some();
    Ok(puzzle(daily, puzzles::order(&state, setup).await?))
}

async fn get_match(
    State(state): State<ServerState>,
    Query(query): Query<PuzzleQuery>,
) -> ApiResult<Response> {
    let setup = setup("match", &query, &state, 5, 10)?;
    let daily = setup.day.is_some();
    Ok(puzzle(daily, puzzles::pick(&state, setup).await?))
}

async fn get_words(
    State(state): State<ServerState>,
    Query(query): Query<PuzzleQuery>,
) -> ApiResult<Response> {
    let setup = setup("words", &query, &state, 1, 1)?;
    let daily = setup.day.is_some();
    Ok(puzzle(daily, puzzles::words(&state, setup).await?))
}

fn puzzle<T: Serialize>(daily: bool, body: T) -> Response {
    if daily {
        response::cached(3_600, body)
    } else {
        response::uncached(body)
    }
}

#[derive(Debug, Deserialize)]
struct RevealQuery {
    t: String,
}

async fn get_reveal(
    State(state): State<ServerState>,
    Query(query): Query<RevealQuery>,
) -> ApiResult<Response> {
    let tokens: Vec<&str> = query
        .t
        .split(',')
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() || tokens.len() > MAX_REVEAL {
        return Err(ApiError::bad_request(format!(
            "ask about 1 to {MAX_REVEAL} pictures at a time"
        )));
    }

    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        out.push(reveal(&state, token).await?);
    }

    Ok(response::cached(3_600, out))
}

#[derive(Debug, Deserialize)]
struct AnswerQuery {
    round: String,
    pick: String,
}

#[derive(Debug, Serialize)]
struct Answer {
    correct: bool,
    answer: Reveal,
}

async fn get_answer(
    State(state): State<ServerState>,
    Query(query): Query<AnswerQuery>,
) -> ApiResult<Response> {
    let answer = token::decode(Kind::Round, &query.round).ok_or(ApiError::NotFound)?;
    let picked = token::decode(Kind::Picture, &query.pick).ok_or(ApiError::NotFound)?;

    let wanted = state.store.picture_dates(answer).await?;
    let chosen = state.store.picture_dates(picked).await?;

    Ok(response::uncached(Answer {
        correct: wanted.first() == chosen.first(),
        answer: reveal(&state, &token::encode(Kind::Picture, answer)).await?,
    }))
}

async fn reveal(state: &ServerState, picture: &str) -> ApiResult<Reveal> {
    let drawn = token::decode(Kind::Picture, picture).ok_or(ApiError::NotFound)?;
    let dates = state.store.picture_dates(drawn).await?;
    let first = dates.first().copied().unwrap_or(drawn);

    let entry = state
        .store
        .entry(first)
        .await?
        .ok_or(ApiError::NotFound)?
        .to_summary();

    Ok(Reveal {
        picture: picture.to_owned(),
        source_url: first.source_url(),
        dates,
        entry,
    })
}

pub async fn get_picture(State(state): State<ServerState>, Path(token): Path<String>) -> Response {
    let Some(date) = token::decode(Kind::Picture, &token) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let path = state.config.thumb_dir.join(date.thumb_path());
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/webp")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(PICTURE_CACHE),
            ),
        ],
        bytes,
    )
        .into_response()
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/date", get(get_date))
        .route("/order", get(get_order))
        .route("/match", get(get_match))
        .route("/words", get(get_words))
        .route("/reveal", get(get_reveal))
        .route("/answer", get(get_answer))
}
