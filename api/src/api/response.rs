use crate::state::Fresh;
use axum::Json;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const JSON: &str = "application/json";

pub fn cached<T: serde::Serialize>(seconds: u64, body: T) -> Response {
    let mut response = Json(body).into_response();

    if let Ok(value) = HeaderValue::from_str(&format!("public, max-age={seconds}")) {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }

    response
}

pub fn revalidated(request: &HeaderMap, fresh: &Fresh, content_type: &'static str) -> Response {
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

fn none_match(request: &HeaderMap, etag: &str) -> bool {
    let Some(header) = request
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    let ours = etag.trim_start_matches("W/");
    header
        .split(',')
        .map(|candidate| candidate.trim())
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/") == ours)
}

pub fn uncached<T: serde::Serialize>(body: T) -> Response {
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
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
    fn sets_the_max_age() {
        let response = cached(86_400, serde_json::json!({}));
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=86400"
        );
    }

    #[test]
    fn a_first_request_gets_the_body_and_a_validator_to_come_back_with() {
        let response = revalidated(&HeaderMap::new(), &fresh(), "application/atom+xml");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(header::ETAG).unwrap(), "\"abc123\"");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=3600"
        );
    }

    #[test]
    fn a_reader_holding_the_current_body_is_told_nothing_changed() {
        let response = revalidated(&asking_for("\"abc123\""), &fresh(), "application/atom+xml");

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers().get(header::ETAG).unwrap(), "\"abc123\"");
    }

    #[test]
    fn a_reader_holding_a_stale_body_gets_the_new_one() {
        let response = revalidated(&asking_for("\"older\""), &fresh(), "application/atom+xml");
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

    #[test]
    fn a_json_body_is_revalidated_the_same_way_an_xml_one_is() {
        let response = revalidated(&asking_for("\"abc123\""), &fresh(), JSON);

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[test]
    fn opts_out_where_freshness_is_the_point() {
        let response = uncached(serde_json::json!({}));
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
}
