use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::WordFilters;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

const TOP_ENTRIES: usize = 10;

#[derive(Debug, Deserialize)]
pub struct WordsQuery {
    q: Option<String>,
    min_total: Option<i64>,
    max_total: Option<i64>,
    sort: Option<String>,
    order: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_words(
    State(state): State<ServerState>,
    Query(query): Query<WordsQuery>,
) -> ApiResult<Response> {
    let filters = WordFilters {
        query: query.q,
        min_total: query.min_total,
        max_total: query.max_total,
    };

    let listing = state
        .store
        .words(
            &filters,
            params::word_order(query.sort.as_deref())?,
            params::order(query.order.as_deref())?,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.list_default_limit,
                state.config.list_max_limit,
            ),
        )
        .await?;

    Ok(response::cached(state.config.cache_list_secs, listing))
}

async fn get_word(
    State(state): State<ServerState>,
    Path(word): Path<String>,
) -> ApiResult<Response> {
    let word = state
        .store
        .word(&word, TOP_ENTRIES)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(response::cached(state.config.cache_list_secs, word))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_words))
        .route("/{word}", get(get_word))
}
