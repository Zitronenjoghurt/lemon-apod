use crate::state::ServerState;
use axum::Router;
use axum::http::{HeaderName, HeaderValue, header};
use axum::routing::get;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

mod health;
mod sitemap;
mod spa;

pub use spa::spa;

const THUMB_CACHE: &str = "public, max-age=31536000, immutable";

const CSP: &str = "default-src 'self'; \
     script-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' https: data:; \
     media-src 'self' https:; \
     font-src 'self'; \
     connect-src 'self' https://api.web3forms.com; \
     frame-src https://www.youtube-nocookie.com https://player.vimeo.com; \
     object-src 'none'; \
     base-uri 'self'; \
     form-action 'self'; \
     frame-ancestors 'none'";

pub fn with_security_headers(router: Router) -> Router {
    fn set(name: HeaderName, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
        SetResponseHeaderLayer::overriding(name, HeaderValue::from_static(value))
    }

    router
        .layer(set(header::CONTENT_SECURITY_POLICY, CSP))
        .layer(set(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(set(header::X_FRAME_OPTIONS, "DENY"))
        .layer(set(
            header::REFERRER_POLICY,
            "strict-origin-when-cross-origin",
        ))
}

pub fn metered() -> Router<ServerState> {
    Router::new().route("/sitemap.xml", get(sitemap::get_sitemap))
}

pub fn unmetered(state: &ServerState) -> Router<ServerState> {
    let thumbs = Router::new()
        .fallback_service(ServeDir::new(&state.config.thumb_dir))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static(THUMB_CACHE),
        ));

    Router::new()
        .route("/health", get(health::get_health))
        .route("/robots.txt", get(sitemap::get_robots))
        .nest("/thumbs", thumbs)
}
