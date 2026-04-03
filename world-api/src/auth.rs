use axum::{
    extract::Request,
    http::Method,
    middleware::Next,
    response::Response,
};

use crate::error::AppError;

// Require auth only on mutation routes.
// Read endpoints remain public for now.
pub async fn require_sim_key(req: Request, next: Next) -> Result<Response, AppError> {
    let method = req.method().clone();

    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let expected = std::env::var("SIM_API_KEY")?;

    let provided = req
        .headers()
        .get("x-sim-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .unwrap_or("");

    if provided.is_empty() {
        return Err(AppError::Unauthorized);
    }

    if provided != expected {
        return Err(AppError::Unauthorized);
    }

    Ok(next.run(req).await)
}
