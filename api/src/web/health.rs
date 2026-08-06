use crate::state::ServerState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub async fn get_health(State(state): State<ServerState>) -> Response {
    match state.store.stats() {
        Ok(stats) => Json(serde_json::json!({
            "status": "ok",
            "entries": stats.entries,
            "latest": stats.latest,
        }))
        .into_response(),
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
