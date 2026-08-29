use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::archive;
use crate::state::ServerState;
use apod_core::Provenance;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct Migration {
    entries: i64,
    provenance: Vec<ProvenanceCount>,
    coverage: Option<archive::Coverage>,
    divergences: Vec<FieldCount>,
    divergent_entries: i64,
    differences: i64,
}

#[derive(Debug, Serialize)]
struct ProvenanceCount {
    provenance: Provenance,
    entries: i64,
}

#[derive(Debug, Serialize)]
struct FieldCount {
    field: String,
    entries: i64,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    field: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_migration(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let migration = state
        .migration
        .get_or_build(|| async {
            let divergences = state.store.divergence_counts().await?;
            let summary = Migration {
                entries: state.store.count().await?,
                provenance: state
                    .store
                    .provenance_counts()
                    .await?
                    .into_iter()
                    .map(|(provenance, entries)| ProvenanceCount {
                        provenance,
                        entries,
                    })
                    .collect(),
                coverage: state.archive.coverage(&state.store).await,
                divergent_entries: state.store.divergent_entries().await?,
                differences: state.store.divergences(None, 0, 0).await?.total,
                divergences: divergences
                    .into_iter()
                    .map(|(field, entries)| FieldCount { field, entries })
                    .collect(),
            };

            serde_json::to_string(&summary).map_err(|error| ApiError::Internal(error.into()))
        })
        .await?;

    Ok(response::revalidated(&headers, &migration, response::JSON))
}

async fn get_divergences(
    State(state): State<ServerState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Response> {
    let listing = state
        .store
        .divergences(
            query.field.as_deref(),
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

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/", get(get_migration))
        .route("/divergences", get(get_divergences))
}
