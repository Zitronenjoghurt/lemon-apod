use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }
}

impl From<apod_core::ApodError> for ApiError {
    fn from(error: apod_core::ApodError) -> Self {
        Self::Internal(error.into())
    }
}

impl From<apod_core::DbError> for ApiError {
    fn from(error: apod_core::DbError) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            Self::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            Self::Internal(error) => {
                tracing::error!("{error:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_owned(),
                )
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(error: ApiError) -> (StatusCode, String) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn reports_client_errors_verbatim() {
        let (status, body) = body_of(ApiError::bad_request("q is required")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("q is required"));
    }

    #[tokio::test]
    async fn hides_internal_detail() {
        let error = ApiError::Internal(anyhow::anyhow!("no such table: entries in /data/apod.db"));
        let (status, body) = body_of(error).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!body.contains("/data/apod.db"), "leaked a path: {body}");
        assert!(body.contains("internal error"));
    }

    #[tokio::test]
    async fn a_feature_that_is_not_running_is_not_an_error_the_caller_caused() {
        let (status, body) = body_of(ApiError::Unavailable("not ready yet".to_owned())).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("not ready yet"));
    }

    #[tokio::test]
    async fn a_vote_budget_reads_as_a_rate_limit_so_the_client_backs_off() {
        let (status, _) = body_of(ApiError::TooManyRequests("come back later".to_owned())).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }
}
