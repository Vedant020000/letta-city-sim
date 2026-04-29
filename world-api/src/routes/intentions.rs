use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::models::intention::{
    AgentIntention, CreateAgentIntentionRequest, UpdateAgentIntentionRequest,
};
use crate::state::AppState;
use crate::ws_events::WorldEventEnvelope;

const ACTIVE_STATUS: &str = "active";

#[derive(Debug, serde::Deserialize)]
pub struct AgentIntentionPath {
    pub id: String,
    pub intention_id: String,
}

pub async fn list_current_intentions(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Vec<AgentIntention>>>> {
    let intentions = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE status = 'active'
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(intentions)))
}

pub async fn list_agent_intentions(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<Vec<AgentIntention>>>> {
    ensure_agent_exists(&state, &agent_id).await?;

    let intentions = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE agent_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(agent_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(intentions)))
}

pub async fn get_current_agent_intention(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<Option<AgentIntention>>>> {
    ensure_agent_exists(&state, &agent_id).await?;

    let intention = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE agent_id = $1 AND status = 'active'
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(intention)))
}

pub async fn create_agent_intention(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentIntentionRequest>,
) -> AppResult<Json<ApiResponse<AgentIntention>>> {
    let summary = required_text(payload.summary, "summary")?;
    let reason = required_text(payload.reason, "reason")?;
    let expected_action = optional_text(payload.expected_action);
    let metadata = payload.metadata.unwrap_or_else(|| serde_json::json!({}));

    let mut tx = state.pool().begin().await?;
    let current_location_id = ensure_agent_exists_for_update(&mut tx, &agent_id).await?;

    let active_exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM agent_intentions
            WHERE agent_id = $1 AND status = 'active'
        )
        "#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if active_exists {
        return Err(AppError::BadRequest(
            "agent already has an active intention".to_string(),
        ));
    }

    let intention = sqlx::query_as::<_, AgentIntention>(
        r#"
        INSERT INTO agent_intentions (
            id, agent_id, summary, reason, status, expected_location_id, expected_action,
            metadata, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, 'active', $5, $6, $7::jsonb, NOW(), NOW())
        RETURNING id, agent_id, summary, reason, status, expected_location_id, expected_action,
                  outcome, metadata, created_at, updated_at, completed_at
        "#,
    )
    .bind(format!("intention_{}", Uuid::new_v4()))
    .bind(&agent_id)
    .bind(summary)
    .bind(reason)
    .bind(payload.expected_location_id)
    .bind(expected_action)
    .bind(metadata.to_string())
    .fetch_one(&mut *tx)
    .await?;

    insert_intention_event(&mut tx, "agent.intention.started", &intention, &current_location_id).await?;
    tx.commit().await?;

    broadcast_intention_event(&state, "agent.intention.started", &intention, current_location_id);

    Ok(Json(ApiResponse::from(intention)))
}

pub async fn update_agent_intention(
    State(state): State<AppState>,
    Path(path): Path<AgentIntentionPath>,
    Json(payload): Json<UpdateAgentIntentionRequest>,
) -> AppResult<Json<ApiResponse<AgentIntention>>> {
    let mut tx = state.pool().begin().await?;
    let current_location_id = ensure_agent_exists_for_update(&mut tx, &path.id).await?;

    let existing = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE id = $1 AND agent_id = $2
        FOR UPDATE
        "#,
    )
    .bind(&path.intention_id)
    .bind(&path.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let summary = match payload.summary {
        Some(value) => required_text(value, "summary")?,
        None => existing.summary,
    };
    let reason = match payload.reason {
        Some(value) => required_text(value, "reason")?,
        None => existing.reason,
    };
    let status = match payload.status {
        Some(value) => normalize_status(value)?,
        None => existing.status,
    };
    let expected_action = payload.expected_action.or(existing.expected_action);
    let expected_location_id = payload.expected_location_id.or(existing.expected_location_id);
    let outcome = payload.outcome.or(existing.outcome);
    let metadata = payload.metadata.unwrap_or(existing.metadata);
    let completed_at_expr = if status == ACTIVE_STATUS {
        None
    } else {
        Some(Utc::now())
    };

    let updated = sqlx::query_as::<_, AgentIntention>(
        r#"
        UPDATE agent_intentions
        SET summary = $1,
            reason = $2,
            status = $3,
            expected_location_id = $4,
            expected_action = $5,
            outcome = $6,
            metadata = $7::jsonb,
            updated_at = NOW(),
            completed_at = $8
        WHERE id = $9 AND agent_id = $10
        RETURNING id, agent_id, summary, reason, status, expected_location_id, expected_action,
                  outcome, metadata, created_at, updated_at, completed_at
        "#,
    )
    .bind(summary)
    .bind(reason)
    .bind(&status)
    .bind(expected_location_id)
    .bind(expected_action)
    .bind(outcome)
    .bind(metadata.to_string())
    .bind(completed_at_expr)
    .bind(&path.intention_id)
    .bind(&path.id)
    .fetch_one(&mut *tx)
    .await?;

    let event_type = event_type_for_status(&updated.status);
    insert_intention_event(&mut tx, event_type, &updated, &current_location_id).await?;
    tx.commit().await?;

    broadcast_intention_event(&state, event_type, &updated, current_location_id);

    Ok(Json(ApiResponse::from(updated)))
}

fn required_text(value: String, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!("{field} cannot be empty")));
    }
    Ok(trimmed.to_string())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    })
}

fn normalize_status(value: String) -> AppResult<String> {
    let status = value.trim().to_lowercase();
    match status.as_str() {
        "active" | "completed" | "failed" | "abandoned" => Ok(status),
        _ => Err(AppError::BadRequest(format!("invalid intention status: {value}"))),
    }
}

fn event_type_for_status(status: &str) -> &'static str {
    match status {
        "completed" => "agent.intention.completed",
        "failed" => "agent.intention.failed",
        "abandoned" => "agent.intention.abandoned",
        _ => "agent.intention.updated",
    }
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

    if !exists {
        return Err(AppError::NotFound);
    }

    Ok(())
}

async fn ensure_agent_exists_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: &str,
) -> AppResult<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn insert_intention_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    intention: &AgentIntention,
    location_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind(event_type)
    .bind(&intention.agent_id)
    .bind(location_id)
    .bind(format!("Agent {} intention: {}", intention.agent_id, intention.summary))
    .bind(intention_event_payload(intention).to_string())
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn broadcast_intention_event(
    state: &AppState,
    event_type: &str,
    intention: &AgentIntention,
    location_id: String,
) {
    let _ = state.event_tx().send(WorldEventEnvelope::new(
        event_type,
        vec![intention.agent_id.clone()],
        Some(location_id),
        intention_event_payload(intention),
    ));
}

fn intention_event_payload(intention: &AgentIntention) -> serde_json::Value {
    serde_json::json!({
        "intention_id": intention.id,
        "agent_id": intention.agent_id,
        "summary": intention.summary,
        "reason": intention.reason,
        "status": intention.status,
        "expected_location_id": intention.expected_location_id,
        "expected_action": intention.expected_action,
        "outcome": intention.outcome,
        "metadata": intention.metadata,
        "created_at": intention.created_at,
        "updated_at": intention.updated_at,
        "completed_at": intention.completed_at,
    })
}
