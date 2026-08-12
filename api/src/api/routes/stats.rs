use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;

async fn get_stats(State(state): State<ServerState>) -> ApiResult<Response> {
    Ok(response::cached(
        state.config.cache_list_secs,
        state.store.stats().await?,
    ))
}

async fn get_timeline(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let timeline = state
        .timeline
        .get_or_build(|| async {
            let timeline = state.store.timeline().await?;
            serde_json::to_string(&timeline).map_err(|error| ApiError::Internal(error.into()))
        })
        .await?;

    Ok(response::revalidated(&headers, &timeline, response::JSON))
}

async fn get_coverage(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult<Response> {
    let coverage = state
        .coverage
        .get_or_build(|| async {
            let coverage = state.store.coverage().await?;
            serde_json::to_string(&coverage).map_err(|error| ApiError::Internal(error.into()))
        })
        .await?;

    Ok(response::revalidated(&headers, &coverage, response::JSON))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_stats))
        .route("/timeline", get(get_timeline))
        .route("/coverage", get(get_coverage))
}
