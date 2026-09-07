use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::apod::Listing;
use apod_core::{ApodSummary, ObjectCount};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ObjectsQuery {
    q: Option<String>,
    catalog: Option<String>,
    sort: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Showing {
    object: ObjectCount,
    items: Vec<ApodSummary>,
}

async fn get_objects(
    State(state): State<ServerState>,
    Query(query): Query<ObjectsQuery>,
) -> ApiResult<Response> {
    let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let catalog = query.catalog.as_deref().filter(|c| !c.is_empty());

    let items = state
        .store
        .objects(
            search,
            catalog,
            params::object_order(query.sort.as_deref())?,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.list_default_limit,
                state.config.list_max_limit,
            ),
        )
        .await?;

    let listing = Listing {
        items,
        total: state.store.object_count(search, catalog).await?,
    };

    Ok(response::cached(state.config.cache_list_secs, listing))
}

async fn get_catalogs(State(state): State<ServerState>) -> ApiResult<Response> {
    let catalogs = state.store.object_catalogs().await?;

    Ok(response::cached(state.config.cache_list_secs, catalogs))
}

async fn get_object(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Query(query): Query<ObjectQuery>,
) -> ApiResult<Response> {
    let object = state.store.object(&id).await?.ok_or(ApiError::NotFound)?;

    let items = state
        .store
        .showing(
            &id,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.list_default_limit,
                state.config.list_max_limit,
            ),
        )
        .await?;

    Ok(response::cached(
        state.config.cache_list_secs,
        Showing { object, items },
    ))
}

async fn get_objects_for(
    State(state): State<ServerState>,
    Path(date): Path<String>,
) -> ApiResult<Response> {
    let objects = state.store.objects_for(params::date(&date)?).await?;

    Ok(response::cached(state.config.cache_list_secs, objects))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_objects))
        .route("/catalogs", get(get_catalogs))
        .route("/entry/{date}", get(get_objects_for))
        .route("/{id}", get(get_object))
}
