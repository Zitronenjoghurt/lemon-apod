use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use apod_core::sky::store::{FeedState, Launch};
use apod_core::sky::weather::WeatherSummary;
use apod_core::sky::{self, SkyNow};
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use chrono::{TimeDelta, Utc};
use serde::Serialize;

const LAUNCHES_BEHIND: i64 = 3;

#[derive(Debug, Serialize)]
struct Sky {
    #[serde(flatten)]
    computed: SkyNow,
    launches: Vec<Launch>,
    weather: Option<WeatherSummary>,
    feeds: Vec<FeedState>,
}

async fn get_sky(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let sky = state.sky.cached.get_or_build(|| build(&state)).await?;

    Ok(response::revalidated(&headers, &sky, response::JSON))
}

async fn build(state: &ServerState) -> ApiResult<String> {
    let now = Utc::now();

    let (launches, weather, feeds) = match state.sky.reader().await {
        Some(reader) => {
            let since = now - TimeDelta::hours(apod_core::sky::store::LAUNCH_LOOKBACK_HOURS);

            let mut launches = reader
                .recent_launches(since, now, LAUNCHES_BEHIND)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading the launches just behind us: {error}");
                    Vec::new()
                });

            launches.extend(
                reader
                    .upcoming_launches(now, state.config.sky_launch_limit)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::warn!("reading upcoming launches: {error}");
                        Vec::new()
                    }),
            );

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

            (launches, weather, feeds)
        }
        None => (Vec::new(), None, Vec::new()),
    };

    serde_json::to_string(&Sky {
        computed: sky::now(now),
        launches,
        weather,
        feeds,
    })
    .map_err(|error| ApiError::Internal(error.into()))
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
        .route("/weather", get(get_weather))
}
