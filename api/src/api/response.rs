use axum::Json;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

pub fn cached<T: serde::Serialize>(seconds: u64, body: T) -> Response {
    let mut response = Json(body).into_response();

    if let Ok(value) = HeaderValue::from_str(&format!("public, max-age={seconds}")) {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }

    response
}

pub fn cached_json(seconds: u64, body: &str) -> Response {
    let mut response = body.to_owned().into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("public, max-age={seconds}")) {
        headers.insert(header::CACHE_CONTROL, value);
    }

    response
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

    #[test]
    fn sets_the_max_age() {
        let response = cached(86_400, serde_json::json!({}));
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=86400"
        );
    }

    #[test]
    fn a_prebuilt_body_is_still_served_as_json() {
        let response = cached_json(300, "{\"years\":[]}");
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=300"
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
