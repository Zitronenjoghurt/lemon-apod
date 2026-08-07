use crate::api::error::ApiResult;
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

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_stats))
}
