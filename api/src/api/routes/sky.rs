use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use apod_core::sky::store::{FeedState, Launch, SpaceWeather};
use apod_core::sky::weather::WeatherSummary;
use apod_core::sky::{self, SkyNow};
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use chrono::Utc;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Sky {
    #[serde(flatten)]
    computed: SkyNow,
    launches: Vec<Launch>,
    space_weather: Option<SpaceWeather>,
    weather: Option<WeatherSummary>,
    feeds: Vec<FeedState>,
}

async fn get_sky(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let sky = state.sky.cached.get_or_build(|| build(&state)).await?;

    Ok(response::revalidated(&headers, &sky, response::JSON))
}

async fn build(state: &ServerState) -> ApiResult<String> {
    let now = Utc::now();

    let (launches, space_weather, weather, feeds) = match state.sky.reader().await {
        Some(reader) => {
            let launches = reader
                .upcoming_launches(now, state.config.sky_launch_limit)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading upcoming launches: {error}");
                    Vec::new()
                });

            let space_weather = reader.space_weather().await.unwrap_or_else(|error| {
                tracing::warn!("reading space weather: {error}");
                None
            });

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

            (launches, space_weather, weather, feeds)
        }
        None => (Vec::new(), None, None, Vec::new()),
    };

    serde_json::to_string(&Sky {
        computed: sky::now(now),
        launches,
        space_weather,
        weather,
        feeds,
    })
    .map_err(|error| ApiError::Internal(error.into()))
}

async fn get_weather(State(state): State<ServerState>) -> ApiResult<Response> {
    let reader = state.sky.reader().await.ok_or(ApiError::NotFound)?;
    let report = reader
        .weather_report()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or(ApiError::NotFound)?;

    Ok(response::cached(state.config.cache_sky_secs, report))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_sky))
        .route("/weather", get(get_weather))
}
