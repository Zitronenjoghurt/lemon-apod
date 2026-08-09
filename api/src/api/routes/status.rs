use crate::api::error::ApiResult;
use crate::api::response;
use crate::config::{Contact, Notify};
use crate::schedule::Schedule;
use crate::state::ServerState;
use apod_core::ApodSummary;
use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Status {
    latest: Option<ApodSummary>,
    entries: i64,
    publish: Schedule,
    contact: Contact,
    notify: Notify,
}

async fn get_status(State(state): State<ServerState>) -> ApiResult<Response> {
    let status = Status {
        latest: state.store.latest().await?.map(|entry| entry.to_summary()),
        entries: state.store.count().await?,
        publish: Schedule::now(&state.config.publish),
        contact: state.config.contact.clone(),
        notify: state.config.notify.clone(),
    };

    Ok(response::cached(state.config.cache_status_secs, status))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_status))
}
