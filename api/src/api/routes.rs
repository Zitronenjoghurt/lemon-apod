use crate::state::ServerState;
use axum::Router;

mod entries;
mod entry;
pub mod games;
pub mod gaps;
mod migration;
mod on_this_day;
mod pictures;
mod random;
mod rating;
mod resources;
mod search;
mod sky;
mod stats;
mod status;
mod words;

pub fn build_routes() -> Router<ServerState> {
    Router::new()
        .nest("/entry", entry::router())
        .nest("/entries", entries::router())
        .nest("/gaps", gaps::router())
        .nest("/migration", migration::router())
        .nest("/search", search::router())
        .nest("/on-this-day", on_this_day::router())
        .nest("/random", random::router())
        .nest("/games", games::router())
        .nest("/status", status::router())
        .nest("/sky", sky::router())
        .nest("/stats", stats::router())
        .nest("/resources", resources::router())
        .nest("/pictures", pictures::router())
        .nest("/words", words::router())
}

/// Kept out of `build_routes` so it can carry its own rate limit.
pub fn rating_routes() -> Router<ServerState> {
    Router::new().nest("/rating", rating::router())
}
