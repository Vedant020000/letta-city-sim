use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::object::{UpdateWorldObjectRequest, WorldObject};
use crate::routes::citizens::enqueue_citizen_wake_tx;
use crate::state::AppState;

pub async fn list_objects_by_location(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<Vec<WorldObject>>> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&location_id)
    .fetch_optional(state.pool())
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let objects = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id = $1
        ORDER BY name
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(objects))
}

pub async fn update_object_state(
    State(state): State<AppState>,
    AgentId(actor_id): AgentId,
    Path(object_id): Path<String>,
    Json(payload): Json<UpdateWorldObjectRequest>,
) -> AppResult<Json<WorldObject>> {
    let mut tx = state.pool().begin().await?;

    let updated_object = sqlx::query_as::<_, WorldObject>(
        r#"
        UPDATE world_objects
        SET state = $1::jsonb
        WHERE id = $2
        RETURNING id, name, location_id, state, actions
        "#,
    )
    .bind(payload.state.to_string())
    .bind(&object_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let description = format!("Object {} state updated", updated_object.id);

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("object.updated")
    .bind(&actor_id)
    .bind(&updated_object.location_id)
    .bind(description)
    .bind(
        serde_json::json!({
            "object_id": updated_object.id,
            "new_state": updated_object.state,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let mut citizen_signal_targets = Vec::new();
    if let Some(location_id) = updated_object.location_id.clone() {
        let targets: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT id
            FROM agents
            WHERE current_location_id = $1
            ORDER BY id
            "#,
        )
        .bind(&location_id)
        .fetch_all(&mut *tx)
        .await?;

        for target_agent_id in targets.iter().filter(|target| target.as_str() != actor_id.as_str()) {
            let enqueued = enqueue_citizen_wake_tx(
                &mut tx,
                target_agent_id,
                "world_event",
                serde_json::json!({
                    "kind": "object",
                    "ref": "object.updated",
                    "details": {
                        "object_id": updated_object.id,
                        "location_id": location_id,
                        "actor_id": actor_id,
                    }
                }),
                format!("{} changed at {}.", updated_object.name, location_id),
                serde_json::json!({
                    "event_type": "object.updated",
                    "object_id": updated_object.id,
                    "object_name": updated_object.name,
                    "location_id": location_id,
                    "state": updated_object.state,
                    "actions": updated_object.actions,
                    "actor_id": actor_id,
                }),
                serde_json::json!([]),
                true,
            )
            .await?;

            if enqueued.should_signal {
                citizen_signal_targets.push(target_agent_id.clone());
            }
        }
    }

    tx.commit().await?;

    for target_agent_id in citizen_signal_targets {
        let _ = state.citizen_signal_tx().send(target_agent_id);
    }

    Ok(Json(updated_object))
}
