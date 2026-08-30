use crate::api::error::ApiResult;
use crate::api::response;
use crate::config::{Contact, Discord, Notify};
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
    rating: Rating,
    contact: Contact,
    notify: Notify,
    discord: Discord,
}

#[derive(Debug, Default, Serialize)]
struct Rating {
    enabled: bool,
    ready: bool,
    pool: u64,
    votes: u64,
    spent: usize,
}

async fn rating(state: &ServerState) -> ApiResult<Rating> {
    let Some(rating) = state.rating.as_deref() else {
        return Ok(Rating::default());
    };

    let mut votes = 0;
    for category in apod_core::rating::Category::ALL {
        votes += rating.store.tally(category).await?.votes;
    }

    Ok(Rating {
        enabled: true,
        ready: rating.ready().await,
        pool: rating.pool_size().await,
        spent: rating.spent_ballots(),
        votes,
    })
}

async fn get_status(State(state): State<ServerState>) -> ApiResult<Response> {
    let status = Status {
        latest: state.store.latest().await?.map(|entry| entry.to_summary()),
        entries: state.store.count().await?,
        publish: Schedule::now(&state.config.publish),
        rating: rating(&state).await?,
        contact: state.config.contact.clone(),
        notify: state.config.notify.clone(),
        discord: state.config.discord.clone(),
    };

    Ok(response::cached(state.config.cache_status_secs, status))
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/", get(get_status))
}
