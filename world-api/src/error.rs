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

    #[error("unexpected error: {0}")]
    Unexpected(String),
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    error: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error"),
            AppError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "configuration error"),
            AppError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "io error"),
            AppError::Unexpected(_) => (StatusCode::INTERNAL_SERVER_ERROR, "unexpected error"),
        };

        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}
