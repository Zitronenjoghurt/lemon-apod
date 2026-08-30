use crate::api::error::{ApiError, ApiResult};
use crate::api::{params, response};
use crate::state::ServerState;
use apod_core::{ApodEntry, FieldDivergence};
use axum::Router;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::get;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Entry {
    #[serde(flatten)]
    entry: ApodEntry,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<FieldDivergence>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    absent: bool,
}

async fn get_latest(State(state): State<ServerState>) -> ApiResult<Response> {
    let entry = state.store.latest().await?.ok_or(ApiError::NotFound)?;
    Ok(response::cached(state.config.cache_latest_secs, entry))
}

async fn get_entry(
    State(state): State<ServerState>,
    Path(date): Path<String>,
) -> ApiResult<Response> {
    let date = params::date(&date)?;
    let entry = state.store.entry(date).await?.ok_or(ApiError::NotFound)?;
    let (changed, absent) = match entry.provenance.has_modern() {
        true => {
            let mut rows = state.store.entry_divergences(date).await?;
            rows.retain(|row| apod_core::is_content(&row.field));
            (rows, false)
        }
        false => (Vec::new(), state.archive.modern_missing(date).await),
    };

    Ok(response::cached(
        state.config.cache_entry_secs,
        Entry {
            entry,
            changed,
            absent,
        },
    ))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/latest", get(get_latest))
        .route("/{date}", get(get_entry))
}
