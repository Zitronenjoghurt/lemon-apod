use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::apod::Listing;
use apod_core::{ApodSummary, Contributor, RoleCount};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreditsQuery {
    q: Option<String>,
    kind: Option<String>,
    sort: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CreditQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Credited {
    contributor: Contributor,
    roles: Vec<RoleCount>,
    items: Vec<CreditedEntry>,
}

#[derive(Debug, Serialize)]
struct CreditedEntry {
    #[serde(flatten)]
    entry: ApodSummary,
    role: String,
}

async fn get_credits(
    State(state): State<ServerState>,
    Query(query): Query<CreditsQuery>,
) -> ApiResult<Response> {
    let kind = params::contributor_kind(query.kind.as_deref())?;
    let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());

    let items = state
        .store
        .contributors(
            search,
            kind,
            params::credit_order(query.sort.as_deref())?,
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
        total: state.store.contributor_count(search, kind).await?,
    };

    Ok(response::cached(state.config.cache_list_secs, listing))
}

async fn get_credit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Query(query): Query<CreditQuery>,
) -> ApiResult<Response> {
    let contributor = state
        .store
        .contributor(&id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let roles = state.store.contributor_roles(&id).await?;

    let items = state
        .store
        .credited(
            &id,
            params::offset(query.offset),
            params::limit(
                query.limit,
                state.config.list_default_limit,
                state.config.list_max_limit,
            ),
        )
        .await?
        .into_iter()
        .map(|(entry, role)| CreditedEntry { entry, role })
        .collect();

    Ok(response::cached(
        state.config.cache_list_secs,
        Credited {
            contributor,
            roles,
            items,
        },
    ))
}

async fn get_credits_for(
    State(state): State<ServerState>,
    Path(date): Path<String>,
) -> ApiResult<Response> {
    let credits = state.store.credits_for(params::date(&date)?).await?;

    Ok(response::cached(state.config.cache_list_secs, credits))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_credits))
        .route("/entry/{date}", get(get_credits_for))
        .route("/{id}", get(get_credit))
}
