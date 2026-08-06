use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
    from: Option<String>,
    to: Option<String>,
    kind: Option<String>,
    copyright: Option<bool>,
    sort: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Response> {
    let Some(q) = query.q.as_deref().filter(|q| !q.trim().is_empty()) else {
        return Err(ApiError::bad_request("q is required"));
    };

    let filters = params::filters(
        query.from.as_deref(),
        query.to.as_deref(),
        query.kind.as_deref(),
        query.copyright,
    )?;

    let results = state.store.search(
        q,
        &filters,
        params::sort_by_date(query.sort.as_deref())?,
        query.offset.unwrap_or(0),
        params::limit(
            query.limit,
            state.config.search_default_limit,
            state.config.search_max_limit,
        ),
        state.config.search_snippet_tokens,
    )?;

    Ok(response::cached(state.config.cache_list_secs, results))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_search))
}
