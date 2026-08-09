use crate::state::{Fresh, ServerState};
use axum::Router;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

mod feed;
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
    Router::new()
        .route("/sitemap.xml", get(sitemap::get_sitemap))
        .route("/atom.xml", get(feed::get_atom))
        .route("/feed.xml", get(feed::get_rss))
}
pub fn cached_xml(request: &HeaderMap, fresh: &Fresh, content_type: &'static str) -> Response {
    let mut response = match none_match(request, &fresh.etag) {
        true => StatusCode::NOT_MODIFIED.into_response(),
        false => fresh.body.to_string().into_response(),
    };

    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&fresh.etag) {
        headers.insert(header::ETAG, value);
    }
    if let Ok(value) =
        HeaderValue::from_str(&format!("public, max-age={}", fresh.max_age.as_secs()))
    {
        headers.insert(header::CACHE_CONTROL, value);
    }

    response
}

pub fn escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;")
}
fn none_match(request: &HeaderMap, etag: &str) -> bool {
    let Some(header) = request
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };

    let ours = etag.trim_start_matches("W/");
    header
        .split(',')
        .map(|candidate| candidate.trim())
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/") == ours)
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
        .route("/pic/{token}", get(crate::api::routes::games::get_picture))
        .nest("/thumbs", thumbs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn fresh() -> Fresh {
        Fresh {
            body: Arc::from("<feed/>"),
            etag: Arc::from("\"abc123\""),
            max_age: Duration::from_secs(3600),
        }
    }

    fn asking_for(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn a_first_request_gets_the_body_and_a_validator_to_come_back_with() {
        let response = cached_xml(&HeaderMap::new(), &fresh(), "application/atom+xml");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(header::ETAG).unwrap(), "\"abc123\"");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=3600"
        );
    }

    #[test]
    fn a_reader_holding_the_current_body_is_told_nothing_changed() {
        let response = cached_xml(&asking_for("\"abc123\""), &fresh(), "application/atom+xml");

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers().get(header::ETAG).unwrap(), "\"abc123\"");
    }

    #[test]
    fn a_reader_holding_a_stale_body_gets_the_new_one() {
        let response = cached_xml(&asking_for("\"older\""), &fresh(), "application/atom+xml");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn the_weak_prefix_readers_add_does_not_defeat_the_match() {
        assert!(none_match(&asking_for("W/\"abc123\""), "\"abc123\""));
    }

    #[test]
    fn one_of_several_offered_validators_is_enough() {
        assert!(none_match(
            &asking_for("\"old\", \"abc123\", \"older\""),
            "\"abc123\""
        ));
        assert!(none_match(&asking_for("*"), "\"abc123\""));
        assert!(!none_match(&asking_for("\"one\", \"two\""), "\"abc123\""));
    }
}
