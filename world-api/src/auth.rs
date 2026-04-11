use axum::{
    async_trait,
    extract::{FromRequestParts, Request},
    http::{Method, request::Parts},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;

use crate::state::AppState;

const HEADER_SIM_KEY: &str = "x-sim-key";
const HEADER_AGENT_ID: &str = "x-agent-id";

#[derive(Clone, Debug)]
pub struct SimKey(String);

impl SimKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct AgentId(String);

impl AgentId {
    pub fn into_inner(self) -> String {
        self.0
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub async fn require_sim_key(req: Request, next: Next) -> Result<Response, AppError> {
    let method = req.method().clone();

    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let expected = std::env::var("SIM_API_KEY")?;

    let provided = req
        .headers()
        .get(HEADER_SIM_KEY)
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

#[async_trait]
impl<S> FromRequestParts<S> for SimKey
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let provided = parts
            .headers
            .get(HEADER_SIM_KEY)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim())
            .unwrap_or("")
            .to_string();

        if provided.is_empty() {
            return Err(AppError::Unauthorized);
        }

        let expected = std::env::var("SIM_API_KEY")?;

        if provided != expected {
            return Err(AppError::Unauthorized);
        }

        Ok(SimKey(provided))
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AgentId {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let value = parts
            .headers
            .get(HEADER_AGENT_ID)
            .ok_or_else(|| AppError::BadRequest("missing x-agent-id header".to_string()))?
            .to_str()
            .map_err(|_| AppError::BadRequest("invalid x-agent-id header".to_string()))?
            .trim();

        if value.is_empty() {
            return Err(AppError::BadRequest(
                "x-agent-id header cannot be empty".to_string(),
            ));
        }

        let exists = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id
            FROM agents
            WHERE id = $1 OR letta_agent_id = $1
            LIMIT 1
            "#,
        )
        .bind(value)
        .fetch_optional(state.pool())
        .await?;

        if exists.is_none() {
            return Err(AppError::NotFound);
        }

        Ok(AgentId(value.to_string()))
    }
}
