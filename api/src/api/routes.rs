use crate::state::ServerState;
use axum::Router;

mod entries;
mod entry;
mod on_this_day;
mod random;
mod search;
mod stats;

pub fn build_routes() -> Router<ServerState> {
    Router::new()
        .nest("/entry", entry::router())
        .nest("/entries", entries::router())
        .nest("/search", search::router())
        .nest("/on-this-day", on_this_day::router())
        .nest("/random", random::router())
        .nest("/stats", stats::router())
}
