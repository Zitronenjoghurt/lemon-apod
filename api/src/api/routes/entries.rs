use crate::api::error::ApiResult;
use crate::api::{params, response};
use crate::state::ServerState;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    from: Option<String>,
    to: Option<String>,
    kind: Option<String>,
    copyright: Option<bool>,
    cursor: Option<String>,
    limit: Option<usize>,
    order: Option<String>,
}

async fn get_entries(
    State(state): State<ServerState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Response> {
    let filters = params::filters(
        query.from.as_deref(),
        query.to.as_deref(),
        query.kind.as_deref(),
        query.copyright,
    )?;

    let page = state.store.list(
        &filters,
        params::optional_date(query.cursor.as_deref())?,
        params::limit(
            query.limit,
            state.config.list_default_limit,
            state.config.list_max_limit,
        ),
        params::order(query.order.as_deref())?,
    )?;

    Ok(response::cached(state.config.cache_list_secs, page))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_entries))
}
