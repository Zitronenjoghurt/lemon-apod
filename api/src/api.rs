use crate::state::ServerState;
use axum::Router;

pub mod error;
pub mod params;
pub mod response;
mod routes;

pub fn build() -> Router<ServerState> {
    Router::new()
        .merge(routes::build_routes())
        .fallback(unknown_route)
}

async fn unknown_route() -> error::ApiError {
    error::ApiError::NotFound
}
