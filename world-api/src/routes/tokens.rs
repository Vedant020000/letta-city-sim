use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{SimKey, hash_agent_token};
use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::state::AppState;

const AGENT_TOKEN_PREFIX: &str = "lcity_agent_";

#[derive(Debug, Deserialize)]
pub struct CreateAgentTokenRequest {
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateAgentTokenResponse {
    pub id: String,
    pub agent_id: String,
    pub token: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AgentTokenSummary {
    pub id: String,
    pub agent_id: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create_agent_token(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentTokenRequest>,
) -> AppResult<Json<ApiResponse<CreateAgentTokenResponse>>> {
    ensure_agent_exists(&state, &agent_id).await?;

    let token = generate_agent_token();
    let token_hash = hash_agent_token(&token);
    let label = payload.label.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let created = sqlx::query_as::<_, AgentTokenSummary>(
        r#"
        INSERT INTO agent_tokens (id, agent_id, token_hash, label, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        RETURNING id, agent_id, label, created_at, last_used_at, revoked_at
        "#,
    )
    .bind(format!("token_{}", Uuid::new_v4()))
    .bind(&agent_id)
    .bind(token_hash)
    .bind(label)
    .fetch_one(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(CreateAgentTokenResponse {
        id: created.id,
        agent_id: created.agent_id,
        token,
        label: created.label,
        created_at: created.created_at,
    })))
}

pub async fn list_agent_tokens(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<Vec<AgentTokenSummary>>>> {
    ensure_agent_exists(&state, &agent_id).await?;

    let tokens = sqlx::query_as::<_, AgentTokenSummary>(
        r#"
        SELECT id, agent_id, label, created_at, last_used_at, revoked_at
        FROM agent_tokens
        WHERE agent_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(agent_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(tokens)))
}

pub async fn revoke_agent_token(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(token_id): Path<String>,
) -> AppResult<Json<ApiResponse<AgentTokenSummary>>> {
    let token = sqlx::query_as::<_, AgentTokenSummary>(
        r#"
        UPDATE agent_tokens
        SET revoked_at = COALESCE(revoked_at, NOW())
        WHERE id = $1
        RETURNING id, agent_id, label, created_at, last_used_at, revoked_at
        "#,
    )
    .bind(token_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(token)))
}

fn generate_agent_token() -> String {
    format!(
        "{}{}{}",
        AGENT_TOKEN_PREFIX,
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

async fn ensure_agent_exists(state: &AppState, agent_id: &str) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(SELECT 1 FROM agents WHERE id = $1)
        "#,
    )
    .bind(agent_id)
    .fetch_one(state.pool())
    .await?;

    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}
