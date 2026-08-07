use crate::state::ServerState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

/// Deliberately outside the rate limiter.
pub async fn get_health(State(state): State<ServerState>) -> Response {
    let built: Result<Arc<str>, apod_core::ApodError> = state
        .health
        .get_or_build(|| async {
            let stats = state.store.stats().await?;
            Ok(serde_json::json!({
                "status": "ok",
                "entries": stats.entries,
                "first": stats.first,
                "latest": stats.latest,
            })
            .to_string())
        })
        .await;

    match built {
        Ok(body) => ok(body),
        Err(error) => {
            tracing::error!("health check failed: {error:#}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "unavailable" })),
            )
                .into_response()
        }
    }
}

fn ok(body: Arc<str>) -> Response {
    let mut response = body.to_string().into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
