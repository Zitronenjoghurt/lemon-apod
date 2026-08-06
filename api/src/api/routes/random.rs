use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RandomQuery {
    kind: Option<String>,
}

async fn get_random(
    State(state): State<ServerState>,
    Query(query): Query<RandomQuery>,
) -> ApiResult<Response> {
    let kind = query.kind.as_deref().map(params::kind).transpose()?;

    let date = state.store.random(kind)?.ok_or(ApiError::NotFound)?;
    let entry = state.store.entry(date)?.ok_or(ApiError::NotFound)?;

    Ok(response::uncached(entry))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_random))
}
