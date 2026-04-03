use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::AppError;
use crate::error::AppResult;
use crate::models::agent::Agent;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateAgentLocationRequest {
    pub location_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentActivityRequest {
    pub activity: String,
}

pub async fn list_agents(State(state): State<AppState>) -> AppResult<Json<Vec<Agent>>> {
    let agents = sqlx::query_as::<_, Agent>(
        r#"
        SELECT
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        FROM agents
        ORDER BY name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(agents))
}

pub async fn get_agent_by_id(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<Agent>> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(agent))
}

pub async fn update_agent_location(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentLocationRequest>,
) -> AppResult<Json<Agent>> {
    let mut tx = state.pool().begin().await?;

    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&payload.location_id)
    .fetch_optional(&mut *tx)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET current_location_id = $1,
            state = 'walking',
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        "#,
    )
    .bind(&payload.location_id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let description = format!(
        "Agent {} moved to location {}",
        updated_agent.id, updated_agent.current_location_id
    );

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.moved")
    .bind(&updated_agent.id)
    .bind(&updated_agent.current_location_id)
    .bind(description)
    .bind("{}")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_agent))
}

pub async fn update_agent_activity(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentActivityRequest>,
) -> AppResult<Json<Agent>> {
    let mut tx = state.pool().begin().await?;

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET current_activity = $1,
            state = 'working',
            activity_started_at = NOW(),
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        "#,
    )
    .bind(&payload.activity)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let description = format!(
        "Agent {} started activity: {}",
        updated_agent.id, payload.activity
    );

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.activity.started")
    .bind(&updated_agent.id)
    .bind(&updated_agent.current_location_id)
    .bind(description)
    .bind("{}")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_agent))
}

pub async fn clear_agent_activity(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<Agent>> {
    let mut tx = state.pool().begin().await?;

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET current_activity = NULL,
            activity_started_at = NULL,
            state = 'idle',
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let description = format!("Agent {} finished current activity", updated_agent.id);

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.activity.finished")
    .bind(&updated_agent.id)
    .bind(&updated_agent.current_location_id)
    .bind(description)
    .bind("{}")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_agent))
}
