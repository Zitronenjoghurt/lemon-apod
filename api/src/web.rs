use crate::state::ServerState;
use axum::Router;
use axum::http::{HeaderValue, header};
use axum::routing::get;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

mod health;
mod sitemap;
mod spa;

pub use spa::spa;

const THUMB_CACHE: &str = "public, max-age=31536000, immutable";

pub fn build(state: &ServerState) -> Router<ServerState> {
    let thumbs = Router::new()
        .fallback_service(ServeDir::new(&state.config.thumb_dir))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static(THUMB_CACHE),
        ));

    Router::new()
        .route("/health", get(health::get_health))
        .route("/robots.txt", get(sitemap::get_robots))
        .route("/sitemap.xml", get(sitemap::get_sitemap))
        .nest("/thumbs", thumbs)
}
