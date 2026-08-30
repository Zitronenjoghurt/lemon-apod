use crate::api::error::ApiResult;
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
    lost: Option<bool>,
    sort: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Response> {
    let q = query.q.as_deref().unwrap_or_default();

    let filters = params::filters(
        query.from.as_deref(),
        query.to.as_deref(),
        query.kind.as_deref(),
        query.copyright,
        query.lost,
    )?;

    let results = state
        .store
        .search(
            q,
            &filters,
            params::sort_by_date(query.sort.as_deref())?,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.search_default_limit,
                state.config.search_max_limit,
            ),
            state.config.search_snippet_tokens,
        )
        .await?;

    Ok(response::cached(state.config.cache_list_secs, results))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_search))
}
