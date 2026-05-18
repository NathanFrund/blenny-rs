// blenny/src/error.rs
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Unified error type for Blenny modules.
#[derive(Debug, thiserror::Error)]
pub enum BlennyError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for BlennyError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            BlennyError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            BlennyError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            BlennyError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let error_type = match &self {
            BlennyError::NotFound(_) => "NotFound",
            BlennyError::Unauthorized(_) => "Unauthorized",
            BlennyError::Internal(_) => "Internal",
        };

        let body = json!({
            "error": {
                "type": error_type,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}
