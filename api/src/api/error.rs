use crate::rating::Budget;
use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

pub type ApiResult<T> = Result<T, ApiError>;

pub const OVER_BUDGET: &str = "over_budget";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("vote budget spent")]
    OverBudget(Budget),
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
            Self::OverBudget(budget) => {
                let mut body = serde_json::json!({
                    "error": self.to_string(),
                    "code": OVER_BUDGET,
                });
                if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(budget)
                    && let Some(object) = body.as_object_mut()
                {
                    object.extend(fields);
                }

                let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
                if let Ok(value) = HeaderValue::from_str(&budget.retry_after.to_string()) {
                    response.headers_mut().insert(header::RETRY_AFTER, value);
                }
                return response;
            }
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

    fn budget(retry_after: u64) -> Budget {
        Budget {
            scope: crate::rating::Scope::Voter,
            allowed: 300,
            window_secs: 3_600,
            retry_after,
        }
    }

    #[tokio::test]
    async fn a_vote_budget_reads_as_a_rate_limit_so_the_client_backs_off() {
        let (status, body) = body_of(ApiError::OverBudget(budget(2_400))).await;

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(body.contains(OVER_BUDGET), "{body}");
        assert!(body.contains("\"retry_after\":2400"), "{body}");
        assert!(
            body.contains("\"allowed\":300") && body.contains("\"window_secs\":3600"),
            "the cap travels with the refusal so the client can say which one was reached \
             without holding a copy of the settings: {body}"
        );
        assert!(
            body.contains("\"scope\":\"voter\""),
            "and whether it was theirs or the one their address shares: {body}"
        );
    }

    #[tokio::test]
    async fn a_spent_budget_sets_the_header_a_client_already_knows_to_read() {
        let response = ApiError::OverBudget(budget(90)).into_response();
        assert_eq!(
            response.headers().get(header::RETRY_AFTER).unwrap(),
            "90",
            "seconds, so a client that only reads the header still waits the right amount"
        );
    }
}
