use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

/// Structured error codes matching the spec
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    Unauthorized,
    PermissionDenied,
    RateLimited,
    InvalidArgument,
    Internal,
    ValidationFailed,
}

impl ErrorCode {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::InvalidArgument => "INVALID_ARGUMENT",
            ErrorCode::Internal => "INTERNAL",
            ErrorCode::ValidationFailed => "VALIDATION_FAILED",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CooperError {
    pub error: CooperErrorBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CooperErrorBody {
    pub code: String,
    pub message: String,
}

impl CooperError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: CooperErrorBody {
                code: code.as_str().to_string(),
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for CooperError {
    fn into_response(self) -> Response {
        let code = match self.error.code.as_str() {
            "NOT_FOUND" => ErrorCode::NotFound,
            "UNAUTHORIZED" => ErrorCode::Unauthorized,
            "PERMISSION_DENIED" => ErrorCode::PermissionDenied,
            "RATE_LIMITED" => ErrorCode::RateLimited,
            "INVALID_ARGUMENT" => ErrorCode::InvalidArgument,
            "VALIDATION_FAILED" => ErrorCode::ValidationFailed,
            _ => ErrorCode::Internal,
        };
        let status = code.status_code();
        let body = serde_json::to_string(&self).unwrap_or_default();
        (status, [("content-type", "application/json")], body).into_response()
    }
}
