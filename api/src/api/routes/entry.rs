use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use axum::Router;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::get;

async fn get_latest(State(state): State<ServerState>) -> ApiResult<Response> {
    let entry = state.store.latest()?.ok_or(ApiError::NotFound)?;
    Ok(response::cached(state.config.cache_latest_secs, entry))
}

async fn get_entry(
    State(state): State<ServerState>,
    Path(date): Path<String>,
) -> ApiResult<Response> {
    let date = params::date(&date)?;
    let entry = state.store.entry(date)?.ok_or(ApiError::NotFound)?;
    Ok(response::cached(state.config.cache_entry_secs, entry))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/latest", get(get_latest))
        .route("/{date}", get(get_entry))
}
