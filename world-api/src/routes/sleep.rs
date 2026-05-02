use axum::{Json, extract::State};
use chrono::Utc;
use serde_json::{Map, Value, json};

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::agent::Agent;
use crate::models::common::{ApiResponse, NotificationMode, NotificationPayload};
use crate::models::object::WorldObject;
use crate::routes::citizens::enqueue_citizen_wake_tx;
use crate::state::AppState;
use crate::ws_events::WorldEventEnvelope;

fn normalize_bed_state(state: &Value) -> Map<String, Value> {
    match state.as_object() {
        Some(map) => map.clone(),
        None => Map::new(),
    }
}

fn occupied_by(state: &Value) -> Option<String> {
    state
        .get("occupied_by")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

async fn location_targets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    location_id: &str,
    agent_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let mut targets = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM agents
        WHERE current_location_id = $1
        ORDER BY id
        "#,
    )
    .bind(location_id)
    .fetch_all(&mut **tx)
    .await?;

    if !targets.iter().any(|target| target == agent_id) {
        targets.push(agent_id.to_string());
    }

    Ok(targets)
}

pub async fn start_sleep(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let mut tx = state.pool().begin().await?;

    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if agent.state == "sleeping" {
        return Err(AppError::BadRequest(
            "agent is already sleeping".to_string(),
        ));
    }

    let beds = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id = $1
          AND 'sleep' = ANY(actions)
        ORDER BY id
        FOR UPDATE
        "#,
    )
    .bind(&agent.current_location_id)
    .fetch_all(&mut *tx)
    .await?;

    let bed = match beds.len() {
        0 => {
            return Err(AppError::BadRequest(
                "no usable bed found in the current location".to_string(),
            ));
        }
        1 => beds.into_iter().next().ok_or_else(|| {
            AppError::BadRequest("no usable bed found in the current location".to_string())
        })?,
        _ => {
            return Err(AppError::BadRequest(
                "multiple usable beds found in the current location; sleep target is ambiguous"
                    .to_string(),
            ));
        }
    };

    if let Some(current_occupant) = occupied_by(&bed.state)
        && current_occupant != agent.id
    {
        return Err(AppError::BadRequest(format!(
            "bed {} is already occupied by {}",
            bed.id, current_occupant
        )));
    }

    let mut bed_state = normalize_bed_state(&bed.state);
    bed_state.insert("occupied_by".to_string(), Value::String(agent.id.clone()));

    sqlx::query(
        r#"
        UPDATE world_objects
        SET state = $1::jsonb
        WHERE id = $2
        "#,
    )
    .bind(Value::Object(bed_state.clone()).to_string())
    .bind(&bed.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE agents
        SET state = 'sleeping',
            current_activity = 'Sleeping',
            activity_started_at = NOW(),
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.sleep.started")
    .bind(&agent.id)
    .bind(&agent.current_location_id)
    .bind(format!("Agent {} went to sleep", agent.id))
    .bind(
        json!({
            "agent_id": agent.id,
            "bed_id": bed.id,
            "location_id": agent.current_location_id,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let targets = location_targets(&mut tx, &agent.current_location_id, &agent.id).await?;
    let mut citizen_signal_targets = Vec::new();

    for target_agent_id in targets.iter().filter(|target| target.as_str() != agent.id.as_str()) {
        let enqueued = enqueue_citizen_wake_tx(
            &mut tx,
            target_agent_id,
            "world_event",
            json!({
                "kind": "sleep",
                "ref": "agent.sleep.started",
                "details": {
                    "agent_id": agent.id,
                    "bed_id": bed.id,
                    "location_id": agent.current_location_id,
                }
            }),
            format!("{} just went to sleep.", agent.name),
            json!({
                "event_type": "agent.sleep.started",
                "agent_id": agent.id,
                "agent_name": agent.name,
                "bed_id": bed.id,
                "bed_name": bed.name,
                "location_id": agent.current_location_id,
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            citizen_signal_targets.push(target_agent_id.clone());
        }
    }

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    for target_agent_id in citizen_signal_targets {
        let _ = state.citizen_signal_tx().send(target_agent_id);
    }

    let _ = state.event_tx().send(WorldEventEnvelope::new(
        "agent.sleep.started",
        targets,
        Some(updated_agent.current_location_id.clone()),
        json!({
            "agent_id": updated_agent.id,
            "bed_id": bed.id,
            "location_id": updated_agent.current_location_id,
        }),
    ));

    Ok(Json(ApiResponse {
        data: updated_agent,
        notification: Some(NotificationPayload {
            message: format!("Went to sleep in {}.", bed.name),
            mode: NotificationMode::Instant,
            eta_seconds: None,
        }),
    }))
}

pub async fn wake_up(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let mut tx = state.pool().begin().await?;

    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if agent.state != "sleeping" {
        return Err(AppError::BadRequest(
            "agent is not currently sleeping".to_string(),
        ));
    }

    let occupied_beds = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE state->>'occupied_by' = $1
        ORDER BY id
        FOR UPDATE
        "#,
    )
    .bind(&agent.id)
    .fetch_all(&mut *tx)
    .await?;

    for bed in &occupied_beds {
        let mut bed_state = normalize_bed_state(&bed.state);
        bed_state.insert("occupied_by".to_string(), Value::Null);

        sqlx::query(
            r#"
            UPDATE world_objects
            SET state = $1::jsonb
            WHERE id = $2
            "#,
        )
        .bind(Value::Object(bed_state).to_string())
        .bind(&bed.id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE agents
        SET state = 'idle',
            current_activity = NULL,
            activity_started_at = NULL,
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .execute(&mut *tx)
    .await?;

    let bed_id = occupied_beds.first().map(|bed| bed.id.clone());

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.sleep.ended")
    .bind(&agent.id)
    .bind(&agent.current_location_id)
    .bind(format!("Agent {} woke up", agent.id))
    .bind(
        json!({
            "agent_id": agent.id,
            "bed_id": bed_id,
            "location_id": agent.current_location_id,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let targets = location_targets(&mut tx, &agent.current_location_id, &agent.id).await?;
    let mut citizen_signal_targets = Vec::new();

    for target_agent_id in targets.iter().filter(|target| target.as_str() != agent.id.as_str()) {
        let enqueued = enqueue_citizen_wake_tx(
            &mut tx,
            target_agent_id,
            "world_event",
            json!({
                "kind": "sleep",
                "ref": "agent.sleep.ended",
                "details": {
                    "agent_id": agent.id,
                    "bed_id": bed_id,
                    "location_id": agent.current_location_id,
                }
            }),
            format!("{} just woke up.", agent.name),
            json!({
                "event_type": "agent.sleep.ended",
                "agent_id": agent.id,
                "agent_name": agent.name,
                "bed_id": bed_id,
                "location_id": agent.current_location_id,
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            citizen_signal_targets.push(target_agent_id.clone());
        }
    }

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    for target_agent_id in citizen_signal_targets {
        let _ = state.citizen_signal_tx().send(target_agent_id);
    }

    let _ = state.event_tx().send(WorldEventEnvelope::new(
        "agent.sleep.ended",
        targets,
        Some(updated_agent.current_location_id.clone()),
        json!({
            "agent_id": updated_agent.id,
            "bed_id": bed_id,
            "location_id": updated_agent.current_location_id,
        }),
    ));

    Ok(Json(ApiResponse {
        data: updated_agent,
        notification: Some(NotificationPayload {
            message: "Woke up and returned to idle.".to_string(),
            mode: NotificationMode::Instant,
            eta_seconds: None,
        }),
    }))
}
