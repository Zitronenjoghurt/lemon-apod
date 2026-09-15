use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use apod_core::sky::store::{FeedState, Launch};
use apod_core::sky::weather::WeatherSummary;
use apod_core::sky::{self, SkyNow};
use apod_core::{ApodDate, ApodSummary};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use chrono::{NaiveTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const DATED_CACHE_SECS: u64 = 30 * 86_400;

const STRIPS: &str = include_str!("../../../../baseline/sky/events.json");
const STRIP_LIMIT: usize = 400;

#[derive(Debug, Serialize)]
struct Sky {
    #[serde(flatten)]
    computed: SkyNow,
    weather: Option<WeatherSummary>,
    feeds: Vec<FeedState>,
}

async fn get_sky(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let sky = state.sky.cached.get_or_build(|| build(&state)).await?;

    Ok(response::revalidated(&headers, &sky, response::JSON))
}

async fn build(state: &ServerState) -> ApiResult<String> {
    let now = Utc::now();

    let (weather, feeds) = match state.sky.reader().await {
        Some(reader) => {
            let weather = reader
                .weather_report()
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading the space weather report: {error}");
                    None
                })
                .map(|report| report.summary(now));

            let feeds = reader.feeds().await.unwrap_or_else(|error| {
                tracing::warn!("reading feed state: {error}");
                Vec::new()
            });

            (weather, feeds)
        }
        None => (None, Vec::new()),
    };

    serde_json::to_string(&Sky {
        computed: sky::now(now),
        weather,
        feeds,
    })
    .map_err(|error| ApiError::Internal(error.into()))
}

async fn get_sky_at(Path(raw): Path<String>) -> ApiResult<Response> {
    let date: ApodDate = crate::api::params::date(&raw)?;
    let at = date
        .naive()
        .and_time(NaiveTime::from_hms_opt(12, 0, 0).expect("midday is a valid time"))
        .and_utc();

    if !sky::in_reach(at) {
        return Err(ApiError::bad_request(format!(
            "the sky can only be worked out between {} and {}",
            sky::FIRST_YEAR,
            sky::LAST_YEAR
        )));
    }

    Ok(response::cached(DATED_CACHE_SECS, sky::now(at)))
}

#[derive(Debug, Serialize)]
struct Launches {
    upcoming: Vec<Launch>,
    flown: Vec<Launch>,
    feeds: Vec<FeedState>,
}

async fn get_launches(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let launches = state
        .sky
        .launches
        .get_or_build(|| build_launches(&state))
        .await?;

    Ok(response::revalidated(&headers, &launches, response::JSON))
}

async fn get_launch(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let reader = state.sky.reader().await.ok_or(ApiError::NotFound)?;
    let launch = reader
        .launch(&id)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or(ApiError::NotFound)?;

    Ok(response::cached(state.config.cache_launches_secs, launch))
}

async fn build_launches(state: &ServerState) -> ApiResult<String> {
    let now = Utc::now();

    let (upcoming, flown, feeds) = match state.sky.reader().await {
        Some(reader) => {
            let history = now - TimeDelta::days(state.config.sky_launch_history_days);

            let upcoming = reader
                .upcoming_launches(now, state.config.sky_launch_limit)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading upcoming launches: {error}");
                    Vec::new()
                });
            let mut flown = reader
                .recent_launches(history, now, state.config.sky_past_launch_limit)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading launches already flown: {error}");
                    Vec::new()
                });
            flown.reverse();

            let feeds = reader.feeds().await.unwrap_or_else(|error| {
                tracing::warn!("reading feed state: {error}");
                Vec::new()
            });

            (upcoming, flown, feeds)
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    };

    serde_json::to_string(&Launches {
        upcoming,
        flown,
        feeds,
    })
    .map_err(|error| ApiError::Internal(error.into()))
}

#[derive(Debug, Deserialize)]
struct StripSpec {
    id: String,
    kind: String,
    title: String,
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Strip {
    id: String,
    kind: String,
    title: String,
    entries: i64,
}

#[derive(Debug, Serialize)]
struct StripEntries {
    id: String,
    title: String,
    entries: Vec<ApodSummary>,
}

static SPECS: LazyLock<Result<Vec<StripSpec>, String>> = LazyLock::new(|| {
    serde_json::from_str(STRIPS).map_err(|error| format!("baseline/sky/events.json: {error}"))
});

async fn get_event_strips(State(state): State<ServerState>) -> ApiResult<Response> {
    let specs = match &*SPECS {
        Ok(specs) => specs,
        Err(problem) => {
            tracing::error!("{problem}");
            return Err(ApiError::Unavailable(
                "the archive links for sky events could not be read".to_owned(),
            ));
        }
    };

    let mut strips = Vec::with_capacity(specs.len());
    for spec in specs {
        strips.push(Strip {
            id: spec.id.clone(),
            kind: spec.kind.clone(),
            title: spec.title.clone(),
            entries: state
                .store
                .count_titles_like(&spec.include, &spec.exclude)
                .await?,
        });
    }

    Ok(response::cached(state.config.cache_list_secs, strips))
}

async fn get_event_strip(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let specs = SPECS.as_ref().map_err(|problem| {
        tracing::error!("{problem}");
        ApiError::Unavailable("the archive links for sky events could not be read".to_owned())
    })?;

    let spec = specs
        .iter()
        .find(|spec| spec.id == id)
        .ok_or(ApiError::NotFound)?;

    let entries = state
        .store
        .titles_like(&spec.include, &spec.exclude, STRIP_LIMIT)
        .await?;

    Ok(response::cached(
        state.config.cache_list_secs,
        StripEntries {
            id: spec.id.clone(),
            title: spec.title.clone(),
            entries,
        },
    ))
}

async fn get_weather(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let weather = state
        .sky
        .weather
        .get_or_build(|| build_weather(&state))
        .await?;

    Ok(response::revalidated(&headers, &weather, response::JSON))
}

async fn build_weather(state: &ServerState) -> ApiResult<String> {
    let reader = state.sky.reader().await.ok_or(ApiError::NotFound)?;
    let mut report = reader
        .weather_report()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or(ApiError::NotFound)?;

    report.scales = report.measured_scales(Utc::now());

    serde_json::to_string(&report).map_err(|error| ApiError::Internal(error.into()))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_sky))
        .route("/at/{date}", get(get_sky_at))
        .route("/launches", get(get_launches))
        .route("/launches/{id}", get(get_launch))
        .route("/events", get(get_event_strips))
        .route("/events/{id}", get(get_event_strip))
        .route("/weather", get(get_weather))
}
