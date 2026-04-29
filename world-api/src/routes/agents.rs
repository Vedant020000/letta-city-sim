use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::auth::AgentId;
use crate::error::AppError;
use crate::error::AppResult;
use crate::models::agent::Agent;
use crate::models::common::{ApiResponse, NotificationMode, NotificationPayload};
use crate::routes::pathfind::compute_shortest_path;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateAgentLocationRequest {
    pub location_id: String,
}

pub async fn agent_health_check(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<AgentHealthResponse>>> {
    let row = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT id, letta_agent_id, current_location_id, state
        FROM agents
        WHERE id = $1 OR letta_agent_id = $1
        LIMIT 1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(AgentHealthResponse {
        status: "ok".to_string(),
        agent_id: row.0,
        letta_agent_id: row.1,
        current_location_id: row.2,
        state: row.3,
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentActivityRequest {
    pub activity: String,
}

#[derive(Debug, Serialize)]
pub struct AgentHealthResponse {
    pub status: String,
    pub agent_id: String,
    pub letta_agent_id: String,
    pub current_location_id: String,
    pub state: String,
}

pub async fn list_agents(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Vec<Agent>>>> {
    let agents = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        ORDER BY name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(agents)))
}

pub async fn get_agent_by_id(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(agent)))
}

pub async fn update_agent_location(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentLocationRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let updated_agent =
        perform_agent_location_update(&state, &agent_id, &payload.location_id).await?;

    Ok(Json(updated_agent))
}

pub async fn move_agent_with_header(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<UpdateAgentLocationRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let result = perform_agent_location_update(&state, &agent_id, &payload.location_id).await?;

    Ok(Json(result))
}

async fn perform_agent_location_update(
    state: &AppState,
    agent_id: &str,
    location_id: &str,
) -> AppResult<ApiResponse<Agent>> {
    let mut tx = state.pool().begin().await?;

    let previous_location_id: Option<String> = sqlx::query_scalar(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut *tx)
    .await?;

    // Broadcast location exit/enter events for WS subscribers (best effort).
    // Routing key for daemon filtering: `agent_targets`.
    // v1 policy: mover + all agents currently in the affected location.
    if let Some(from_loc) = previous_location_id.clone() {
        if from_loc != location_id {
            let mut from_targets: Vec<String> = sqlx::query_scalar::<_, String>(
                r#"
                SELECT id
                FROM agents
                WHERE current_location_id = $1
                "#,
            )
            .bind(&from_loc)
            .fetch_all(&mut *tx)
            .await?;

            if !from_targets.contains(&agent_id.to_string()) {
                from_targets.push(agent_id.to_string());
            }

            let _ = state
                .event_tx()
                .send(crate::ws_events::WorldEventEnvelope::new(
                    "location.exit",
                    from_targets,
                    Some(from_loc.clone()),
                    serde_json::json!({
                        "agent_id": agent_id,
                        "from_location_id": from_loc,
                        "to_location_id": location_id,
                    }),
                ));
        }
    }

    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(location_id)
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
        RETURNING *
        "#,
    )
    .bind(location_id)
    .bind(agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut to_targets: Vec<String> = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM agents
        WHERE current_location_id = $1
        "#,
    )
    .bind(&updated_agent.current_location_id)
    .fetch_all(&mut *tx)
    .await?;

    if !to_targets.contains(&updated_agent.id) {
        to_targets.push(updated_agent.id.clone());
    }

    let _ = state
        .event_tx()
        .send(crate::ws_events::WorldEventEnvelope::new(
            "location.enter",
            to_targets,
            Some(updated_agent.current_location_id.clone()),
            serde_json::json!({
                "agent_id": updated_agent.id,
                "from_location_id": previous_location_id,
                "to_location_id": updated_agent.current_location_id,
            }),
        ));

    let path_resp = compute_shortest_path(state.pool(), agent_id, location_id).await;
    let travel_seconds = match path_resp {
        Ok(resp) => resp.travel_time_seconds,
        Err(_) => 0,
    };

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

    let notification_mode = if travel_seconds <= 15 {
        NotificationMode::Instant
    } else {
        NotificationMode::Deferred
    };

    let notification = NotificationPayload {
        message: if matches!(notification_mode, NotificationMode::Instant) {
            format!("Arrived at {}.", updated_agent.current_location_id)
        } else {
            format!(
                "Walking to {} (ETA ~{}s)",
                updated_agent.current_location_id, travel_seconds
            )
        },
        mode: notification_mode.clone(),
        eta_seconds: if matches!(notification_mode, NotificationMode::Deferred) {
            Some(travel_seconds as u64)
        } else {
            None
        },
    };

    Ok(ApiResponse {
        data: updated_agent,
        notification: Some(notification),
    })
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
        RETURNING *
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
        RETURNING *
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
