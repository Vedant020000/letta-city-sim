use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State as AxumState},
    http::{Method, header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

const HEADER_SIM_KEY: &str = "x-sim-key";
const HEADER_AGENT_ID: &str = "x-agent-id";
const AGENT_TOKEN_PREFIX: &str = "lcity_agent_";

#[derive(Clone, Debug)]
pub struct SimKey(pub String);

impl SimKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn into_inner(self) -> String {
        self.0
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct AuthContext {
    agent_id: Option<String>,
    is_admin: bool,
}

impl AuthContext {
    pub fn admin() -> Self {
        Self {
            agent_id: None,
            is_admin: true,
        }
    }

    pub fn agent(agent_id: String) -> Self {
        Self {
            agent_id: Some(agent_id),
            is_admin: false,
        }
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }

    pub fn ensure_agent(&self, expected_agent_id: &str) -> AppResult<()> {
        if self.is_admin {
            return Ok(());
        }

        match self.agent_id.as_deref() {
            Some(agent_id) if agent_id == expected_agent_id => Ok(()),
            Some(_) => Err(AppError::Forbidden),
            None => Err(AppError::Unauthorized),
        }
    }
}

pub fn hash_agent_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub async fn require_sim_key(
    AxumState(state): AxumState<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = req.method().clone();

    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    if request_has_valid_sim_key(req.headers())? {
        req.extensions_mut().insert(AuthContext::admin());
        return Ok(next.run(req).await);
    }

    let bearer = bearer_token(req.headers());
    if let Some(token) = bearer {
        let auth = resolve_bearer_token(Some(&state), token).await?;
        req.extensions_mut().insert(auth);
        return Ok(next.run(req).await);
    }

    Err(AppError::Unauthorized)
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
impl FromRequestParts<AppState> for AuthContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(auth) = parts.extensions.get::<AuthContext>() {
            return Ok(auth.clone());
        }

        if request_has_valid_sim_key(&parts.headers)? {
            return Ok(AuthContext::admin());
        }

        if let Some(token) = bearer_token(&parts.headers) {
            return resolve_bearer_token(Some(state), token).await;
        }

        Err(AppError::Unauthorized)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AgentId {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(auth) = parts.extensions.get::<AuthContext>() {
            if let Some(agent_id) = auth.agent_id() {
                reject_mismatched_agent_header(&parts.headers, agent_id)?;
                return Ok(AgentId(agent_id.to_string()));
            }
        }

        if let Some(token) = bearer_token(&parts.headers) {
            let auth = resolve_bearer_token(Some(state), token).await?;
            if let Some(agent_id) = auth.agent_id() {
                reject_mismatched_agent_header(&parts.headers, agent_id)?;
                parts.extensions.insert(auth.clone());
                return Ok(AgentId(agent_id.to_string()));
            }
        }

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

fn request_has_valid_sim_key(headers: &axum::http::HeaderMap) -> AppResult<bool> {
    let provided = headers
        .get(HEADER_SIM_KEY)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .unwrap_or("");

    if provided.is_empty() {
        return Ok(false);
    }

    let expected = std::env::var("SIM_API_KEY")?;
    Ok(provided == expected)
}

fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let value = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?
        .trim();

    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

async fn resolve_bearer_token(state: Option<&AppState>, token: &str) -> AppResult<AuthContext> {
    if !token.starts_with(AGENT_TOKEN_PREFIX) {
        return Err(AppError::Unauthorized);
    }

    let state = state.ok_or(AppError::Unauthorized)?;
    let token_hash = hash_agent_token(token);

    let agent_id = sqlx::query_scalar::<_, String>(
        r#"
        UPDATE agent_tokens
        SET last_used_at = NOW()
        WHERE token_hash = $1
          AND revoked_at IS NULL
        RETURNING agent_id
        "#,
    )
    .bind(token_hash)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::Unauthorized)?;

    Ok(AuthContext::agent(agent_id))
}

fn reject_mismatched_agent_header(
    headers: &axum::http::HeaderMap,
    resolved_agent_id: &str,
) -> AppResult<()> {
    let Some(header_agent_id) = headers
        .get(HEADER_AGENT_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    if header_agent_id == resolved_agent_id {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}
