use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("configuration error: {0}")]
    Config(#[from] std::env::VarError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("resource not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("unexpected error: {0}")]
    Unexpected(String),
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message, detail) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error", None),
            AppError::Config(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration error",
                None,
            ),
            AppError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "io error", None),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found", None),
            AppError::BadRequest(detail) => (
                StatusCode::BAD_REQUEST,
                "bad request",
                Some(detail.as_str()),
            ),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", None),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", None),
            AppError::Unexpected(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "unexpected error", None)
            }
        };

        let body = Json(ErrorResponse {
            error: message,
            detail,
        });
        (status, body).into_response()
    }
}
