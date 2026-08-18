use crate::state::ServerState;
use axum::Router;

pub mod error;
pub mod params;
pub mod response;
pub mod routes;

pub fn read_routes() -> Router<ServerState> {
    routes::build_routes()
}

pub fn vote_routes() -> Router<ServerState> {
    routes::rating_routes()
}

pub async fn unknown_route() -> error::ApiError {
    error::ApiError::NotFound
}
