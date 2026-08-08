use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use apod_core::sky::store::{FeedState, Launch, SpaceWeather};
use apod_core::sky::{self, SkyNow};
use axum::Router;
use axum::extract::State;
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
    feeds: Vec<FeedState>,
}

async fn get_sky(State(state): State<ServerState>) -> ApiResult<Response> {
    let body = state.sky.cached.get_or_build(|| build(&state)).await?;

    Ok(response::cached_json(state.config.cache_sky_secs, &body))
}

async fn build(state: &ServerState) -> ApiResult<String> {
    let now = Utc::now();

    let (launches, space_weather, feeds) = match state.sky.reader().await {
        Some(reader) => {
            let launches = reader
                .upcoming_launches(now, state.config.sky_launch_limit)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("reading upcoming launches: {error}");
                    Vec::new()
                });

            let weather = reader.space_weather().await.unwrap_or_else(|error| {
                tracing::warn!("reading space weather: {error}");
                None
            });

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
        space_weather,
        feeds,
    })
    .map_err(|error| ApiError::Internal(error.into()))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_sky))
}
