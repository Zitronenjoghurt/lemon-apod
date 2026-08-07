use crate::api::error::ApiResult;
use crate::api::{params, response};
use crate::state::ServerState;
use axum::Router;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::get;

async fn get_on_this_day(
    State(state): State<ServerState>,
    Path(month_day): Path<String>,
) -> ApiResult<Response> {
    let (month, day) = params::month_day(&month_day)?;
    let items = state.store.on_this_day(month, day).await?;

    Ok(response::cached(state.config.cache_list_secs, items))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/{month_day}", get(get_on_this_day))
}
