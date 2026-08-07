use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::ResourceFilters;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;

const HOST_LIMIT: usize = 200;

#[derive(Debug, Deserialize)]
pub struct CatalogueQuery {
    q: Option<String>,
    host: Option<String>,
    min_refs: Option<i64>,
    credited: Option<bool>,
    sort: Option<String>,
    order: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RefsQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_resources(
    State(state): State<ServerState>,
    Query(query): Query<CatalogueQuery>,
) -> ApiResult<Response> {
    let filters = ResourceFilters {
        query: query.q,
        host: query.host,
        min_refs: query.min_refs,
        credited: query.credited,
    };

    let listing = state
        .store
        .resources(
            &filters,
            params::resource_order(query.sort.as_deref())?,
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

async fn get_hosts(State(state): State<ServerState>) -> ApiResult<Response> {
    Ok(response::cached(
        state.config.cache_list_secs,
        state.store.resource_hosts(HOST_LIMIT).await?,
    ))
}

async fn get_resource(
    State(state): State<ServerState>,
    Path(id): Path<i64>,
    Query(query): Query<RefsQuery>,
) -> ApiResult<Response> {
    let resource = state
        .store
        .resource(
            id,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.list_default_limit,
                state.config.list_max_limit,
            ),
        )
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(response::cached(state.config.cache_list_secs, resource))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_resources))
        .route("/hosts", get(get_hosts))
        .route("/{id}", get(get_resource))
}
