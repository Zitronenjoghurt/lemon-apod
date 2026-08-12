use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::PictureFilters;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PicturesQuery {
    q: Option<String>,
    min_appearances: Option<i64>,
    retitled: Option<bool>,
    sort: Option<String>,
    order: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_pictures(
    State(state): State<ServerState>,
    Query(query): Query<PicturesQuery>,
) -> ApiResult<Response> {
    let filters = PictureFilters {
        query: query.q,
        min_appearances: query.min_appearances,
        retitled: query.retitled,
    };

    let listing = state
        .store
        .pictures(
            &filters,
            params::picture_order(query.sort.as_deref())?,
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

async fn get_picture(
    State(state): State<ServerState>,
    Path(date): Path<String>,
) -> ApiResult<Response> {
    let appearances = state
        .store
        .picture_appearances(params::date(&date)?)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(response::cached(state.config.cache_list_secs, appearances))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_pictures))
        .route("/{date}", get(get_picture))
}
