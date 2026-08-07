use crate::api::error::{ApiError, ApiResult};
use crate::api::response;
use crate::state::ServerState;
use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;

async fn get_stats(State(state): State<ServerState>) -> ApiResult<Response> {
    Ok(response::cached(
        state.config.cache_list_secs,
        state.store.stats().await?,
    ))
}

async fn get_timeline(State(state): State<ServerState>) -> ApiResult<Response> {
    let body = state
        .timeline
        .get_or_build(|| async {
            let timeline = state.store.timeline().await?;
            serde_json::to_string(&timeline).map_err(|error| ApiError::Internal(error.into()))
        })
        .await?;

    Ok(response::cached_json(state.config.cache_list_secs, &body))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_stats))
        .route("/timeline", get(get_timeline))
}
